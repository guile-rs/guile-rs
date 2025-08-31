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

use {
    crate::GuileVM,
    guile_sys::{scm_dynwind_begin, scm_dynwind_end, scm_dynwind_unwind_handler},
    std::{ffi::c_void, marker::PhantomData, pin::Pin, ptr},
};

/// # Safety
///
/// - `ptr` must be of type `T`.
/// - All preconditions of [ptr::drop_in_place].
unsafe extern "C" fn cast_drop_in_place<T>(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            ptr.cast::<T>().drop_in_place();
        }
    }
}

pub struct Dynwind<'vm> {
    _marker: PhantomData<&'vm ()>,
}
impl<'vm> Dynwind<'vm> {
    /// # Safety
    ///
    /// This must be dropped.
    unsafe fn new(_: &'vm GuileVM) -> Self {
        // SAFETY: The reference if proof of being in guile mode and we put dropping in the preconditions.
        unsafe {
            scm_dynwind_begin(0);
        }

        Self {
            _marker: PhantomData,
        }
    }

    /// Call drop for a reference if stack unwinding occurs.
    ///
    /// # Safety
    ///
    /// You must make sure that the pointee of `ptr` has a lifetime lesser than equal to the lifetime of the [Dynwind] object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::pin::Pin;
    /// guile::init(|vm| {
    ///     vm.dynwind_scope(|dynwind| {
    ///         let mut pointee = Vec::<i32>::new();
    ///         let ptr = Pin::new(&mut pointee);
    ///         dynwind.protect(ptr);
    ///     });
    /// });
    /// ```
    pub fn protect<T>(&mut self, ptr: Pin<&mut T>) {
        unsafe {
            scm_dynwind_unwind_handler(
                // SAFETY: `ptr` is a reference of type `T`
                // SAFETY: the pointer is valid for both read and write
                // SAFETY: the function checks for null
                // SAFETY: all modifications to `ptr` should still keep invariants alive
                // SAFETY: the destructors get ran before stack unwinding, making them unaccessible afterwards
                Some(cast_drop_in_place::<T>),
                // SAFETY: we don't move this
                ptr::from_mut(ptr.get_unchecked_mut()).cast(),
                // 0 since `SCM_F_WIND_EXPLICITLY` would cause a double free when `T` gets dropped.
                0,
            );
        }
    }
}
impl Drop for Dynwind<'_> {
    fn drop(&mut self) {
        // SAFETY: the lifetime is proof of being in guile mode and `Self::new` is the only way to create this type, which runs `scm_dynwind_begin`
        unsafe {
            scm_dynwind_end();
        }
    }
}

impl GuileVM {
    /// Create a [Dynwind] raii guard that you can use to prevent memory leaks when unwinding.
    ///
    /// # Examples
    ///
    /// ```
    /// # use guile::GuileVM;
    /// # use guile_sys::{scm_from_uint8, scm_misc_error, scm_make_list};
    /// # use std::pin::Pin;
    /// # use std::sync::atomic::{self, AtomicBool};
    /// static DROPPED: AtomicBool = AtomicBool::new(false);
    ///
    /// fn unwind(_: &GuileVM) -> ! {
    ///     let zero = unsafe { scm_from_uint8(0) };
    ///     unsafe { scm_misc_error(c"unwind".as_ptr(), c"start unwinding".as_ptr(), scm_make_list(zero, zero)); }
    ///     unreachable!()
    /// }
    ///
    /// struct SetTrue;
    /// impl Drop for SetTrue {
    ///     fn drop(&mut self) {
    ///         DROPPED.store(true, atomic::Ordering::Release);
    ///     }
    /// }
    ///
    /// guile::init(|vm| {
    ///     let _guard = SetTrue;
    /// });
    /// assert!(DROPPED.load(atomic::Ordering::Acquire), "the destructor should run as normal");
    ///
    /// DROPPED.store(false, atomic::Ordering::Release);
    /// guile::init(|vm| {
    ///     let _guard = SetTrue;
    ///     unwind(&vm);
    /// });
    /// assert_eq!(DROPPED.load(atomic::Ordering::Acquire), false, "the unwinding prevented the destructor from running");
    ///
    /// guile::init(|vm| {
    ///     vm.dynwind_scope(|dynwind| {
    ///         let mut guard = SetTrue;
    ///         let guard = Pin::new(&mut guard);
    ///         dynwind.protect(guard);
    ///         unwind(&vm);
    ///     });
    /// });
    /// assert!(DROPPED.load(atomic::Ordering::Acquire), "the dynwind should have ran the destructor");
    /// ```
    pub fn dynwind_scope<'vm, F, O>(&'vm self, f: F) -> O
    where
        F: FnOnce(&mut Dynwind<'vm>) -> O,
    {
        // SAFETY: we are in guile mode from the `self` reference and [guile::init] runs the closure in an `extern "C"` function which aborts upon panics, making the destructor _always_ run.
        let mut dynwind = unsafe { Dynwind::new(self) };

        f(&mut dynwind)
    }
}
