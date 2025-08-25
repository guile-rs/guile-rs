// Copyright 2016 David Li

// This file is part of guile-rs.

// guile-rs is free software: you can redistribute it and/or modify it
// under the terms of the GNU Lesser General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.

// guile-rs is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.

// You should have received a copy of the GNU Lesser General Public
// License along with guile-rs.  If not, see
// <http://www.gnu.org/licenses/>.
extern crate guile_sys;
extern crate libc;

use libc::{c_char, c_void};
use std::{
    ffi, ptr,
    sync::{
        atomic::{self, AtomicBool},
        Mutex,
    },
    thread_local,
};

/// Lock for global initalization since guile cannot initialize multiple threads at the same time.
static INITIALIZATION_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    /// Whether or not the current thread has been initialized.
    static INITIALIZED: AtomicBool = const { AtomicBool::new(false) };
    /// Whether or not the current thread is currently in guile mode.
    static GUILE_MODE: AtomicBool = const { AtomicBool::new(false) };
}

pub struct GuileVM {}

// TODO: link documentation once catching and dynwind is implemented
/// Attempt to run `func` with access to the guile vm.
///
/// # Non-local exits
///
/// The result of the function will only be returned if it has not exitted non-locally in guile.
///
/// This would only apply to the top level guile mode entry point. If you would like to protect against
/// non-local exits, consider using a catch block or dynwind.
///
/// # Examples
///
/// ```
/// # use guile::GuileVM;
/// # // TODO: create and use safe abstractions for theses
/// # use guile_sys::{scm_throw, scm_from_utf8_symbol, scm_make_list, scm_from_uint8};
/// fn intentional_throw(_: &GuileVM) -> ! {
///     // SAFETY: bindgen should provide the correct type signatures, making this safe.
///     // SAFETY: the guile vm is proof of being in guile mode.
///     let zero = unsafe { scm_from_uint8(0) };
///     unsafe {
///         scm_throw(scm_from_utf8_symbol(c"foo".as_ptr()), scm_make_list(zero, zero));
///     }
///     unreachable!()
/// }
/// assert_eq!(guile::init(|_| {}), Some(()));
/// assert_eq!(guile::init(|vm| {
///     intentional_throw(vm)
/// }), None);
/// assert_eq!(guile::init(|guile| {
///     drop(guile); // oops
///
///     assert_eq!(guile::init(|vm| {
///         intentional_throw(vm)
///     }), unreachable!("this never gets ran"));
/// }), None, "the throw should be caught here");
/// ```
pub fn init<F, O>(func: F) -> Option<O>
where
    F: FnOnce(&mut GuileVM) -> O,
{
    if GUILE_MODE.with(|local_init| local_init.load(atomic::Ordering::Acquire)) {
        Some(func(&mut GuileVM {}))
    } else {
        let _lock = INITIALIZED
            .with(|initialized| !initialized.load(atomic::Ordering::Acquire))
            .then(|| INITIALIZATION_LOCK.lock().unwrap());

        let mut data = WithGuileCallbackData {
            closure: Some(func),
            output: None,
        };
        unsafe {
            guile_sys::scm_with_guile(
                Some(with_guile_callback::<F, O>),
                (&raw mut data).cast::<c_void>(),
            );
        }

        GUILE_MODE.with(|initialized| initialized.store(false, atomic::Ordering::Release));

        data.output
    }
}
struct WithGuileCallbackData<F, O>
where
    F: FnOnce(&mut GuileVM) -> O,
{
    closure: Option<F>,
    output: Option<O>,
}

/// Callback for use by [guile_sys::scm_with_guile]
///
/// # Safety
///
/// `ptr` must be a pointer of type `WithGuileCallbackData<F, O>`
unsafe extern "C" fn with_guile_callback<F, O>(ptr: *mut c_void) -> *mut c_void
where
    F: FnOnce(&mut GuileVM) -> O,
{
    INITIALIZED.with(|local_init| local_init.store(true, atomic::Ordering::Release));
    GUILE_MODE.with(|local_init| local_init.store(true, atomic::Ordering::Release));

    let data = ptr.cast::<WithGuileCallbackData<F, O>>();
    if let Some(data) = unsafe { data.as_mut() } {
        data.output = data
            .closure
            .take()
            .map(|closure| (closure)(&mut GuileVM {}));
    }

    ptr::null_mut()
}

impl GuileVM {
    /// Run a blocking operation without impacting garbage collection.
    pub fn block<F, O>(&mut self, operation: F) -> O
    where
        F: FnOnce() -> O,
    {
        let mut data = WithoutGuileCallbackData {
            operation: Some(operation),
            output: None,
        };

        unsafe {
            guile_sys::scm_without_guile(
                Some(without_guile_callback::<F, O>),
                (&raw mut data).cast(),
            );
        }

        GUILE_MODE.with(|local_init| local_init.store(true, atomic::Ordering::Release));

        data.output.unwrap()
    }

    pub fn shell(&self, args: Vec<String>) {
        unsafe {
            let mut argv: Vec<*mut c_char> = args
                .into_iter()
                .map(|arg| ffi::CString::new(arg).unwrap().into_raw())
                .collect();
            let argv_ptr = argv.as_mut_ptr();
            guile_sys::scm_shell(argv.len() as i32, argv_ptr);
        }
    }
}

struct WithoutGuileCallbackData<F, O>
where
    F: FnOnce() -> O,
{
    operation: Option<F>,
    output: Option<O>,
}
unsafe extern "C" fn without_guile_callback<F, O>(data: *mut c_void) -> *mut c_void
where
    F: FnOnce() -> O,
{
    GUILE_MODE.with(|local_init| local_init.store(false, atomic::Ordering::Release));

    let data = data.cast::<WithoutGuileCallbackData<F, O>>();
    if let Some(data) = unsafe { data.as_mut() } {
        data.output = data.operation.take().map(|operation| (operation)());
    }

    ptr::null_mut()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn in_and_out() {
        assert!(init(|api| api.block(|| init(|_| true))).unwrap().unwrap());
    }

    #[test]
    fn multi_threading() {
        let spawn_thread = || std::thread::spawn(|| init(|_| {}).unwrap());
        let (thread_1, thread_2) = (spawn_thread(), spawn_thread());

        thread_1.join().unwrap();
        thread_2.join().unwrap();
    }

    #[test]
    fn mutex_deadlock() {
        assert!(init(|_| init(|_| true)).unwrap().unwrap());
    }

    #[test]
    fn it_works() {
        init(|_| {
            println!("Hello guile!");
        });
    }
}
