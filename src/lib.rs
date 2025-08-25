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

use std::ffi::{self, c_char, c_void};

pub struct GuileVM {}

pub fn init<F>(func: F)
where
    F: Fn(GuileVM),
{
    unsafe {
        guile_sys::scm_with_guile(
            Some(with_guile_callback::<F>),
            &func as *const _ as *mut c_void,
        );
    }
}

impl GuileVM {
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

unsafe extern "C" fn with_guile_callback<F>(data: *mut c_void) -> *mut c_void
where
    F: Fn(GuileVM),
{
    let callback = data as *mut F;

    let vm = GuileVM {};

    (*callback)(vm);

    std::ptr::null_mut()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_works() {
        init(|_| {
            println!("Hello guile!");
        });
    }
}
