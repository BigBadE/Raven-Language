use std::ffi::c_char;
use std::mem::size_of;
use std::ptr;

use crate::RavenExtern;

impl RavenExtern for String {
    type Input = c_char;

    unsafe fn translate(raven_type: *mut c_char) -> Self {
        let mut output = vec![];
        let mut pointer = raven_type;
        loop {
            let value = ptr::read(pointer);
            if value == 0 {
                break;
            }
            output.push(value as u8);
            pointer = pointer.add(1);
        }
        return String::from_utf8_unchecked(output);
    }
}

impl RavenExtern for bool {
    type Input = bool;

    unsafe fn translate(raven_type: *mut bool) -> Self {
        return ptr::read(raven_type);
    }
}

impl<T: RavenExtern> RavenExtern for Vec<T> {
    type Input = ();

    unsafe fn translate(raven_type: *mut ()) -> Self {
        return load_array(raven_type);
    }
}

/// Loads a raw array into a Vec
fn load_raw<T: RavenExtern>(length: u64, pointer: *mut T) -> Vec<T> {
    let mut output = Vec::new();
    let offset = size_of::<T::Input>() as u64;
    let mut pointer = pointer as *mut u64;
    for _ in 0..length {
        output.push(unsafe { T::translate(ptr::read(pointer) as *mut T::Input) });
        pointer = (pointer as u64 + offset) as *mut u64;
    }

    return output;
}

/// Loads an array from a pointer into a Vec
fn load_array<T: RavenExtern>(ptr: *mut ()) -> Vec<T> {
    let len = unsafe { ptr::read(ptr as *mut u64) };
    return load_raw(len, (ptr as u64 + (size_of::<T::Input>()) as u64) as *mut T);
}

impl RavenExtern for () {
    type Input = ();

    unsafe fn translate(_: *mut ()) -> Self {
        return ();
    }
}
