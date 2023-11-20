use core::fmt::Debug;
use std::ffi::{c_char, c_int, CString};
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

/// A raw project from Raven
#[derive(Debug)]
#[repr(C, align(8))]
pub struct RawRavenProject {
    /// Field on every Raven type
    type_id: c_int,
    /// Project name
    pub name: AtomicPtr<c_char>,
    /// Project dependencies
    pub dependencies: AtomicPtr<RawArray>,
}

/// A raw Raven array
#[derive(Debug)]
pub struct RawArray {}

/// A raw Raven Dependency
#[repr(C, align(8))]
#[derive(Debug)]
pub struct RawDependency {
    /// Field on every Raven type
    type_id: c_int,
    /// Dependency name
    pub name: AtomicPtr<c_char>,
}

/// A Raven project
#[derive(Debug)]
pub struct RavenProject {
    /// Project name
    pub name: String,
    /// Project dependencies
    pub dependencies: Vec<Dependency>,
}

/// A Raven project dependency
#[derive(Debug)]
pub struct Dependency {
    /// Dependency name
    pub name: String,
}

/// Loads a raw array into a Vec
fn load_raw<T: Debug>(length: u64, pointer: *mut T) -> Vec<T> {
    let mut output = Vec::new();
    let offset = size_of::<T>() as u64;
    let mut pointer = pointer as *mut u64;

    for _ in 0..length {
        output.push(unsafe { ptr::read(ptr::read(pointer) as *mut T) });
        pointer = (pointer as u64 + offset) as *mut u64;
    }

    return output;
}

impl From<RawRavenProject> for RavenProject {
    fn from(value: RawRavenProject) -> Self {
        unsafe {
            return Self {
                name: CString::from_raw(value.name.load(Ordering::Relaxed)).to_str().unwrap().to_string(),
                dependencies: load_array(value.dependencies)
                    .into_iter()
                    .map(|inner: RawDependency| Dependency::from(inner))
                    .collect::<Vec<_>>(),
            };
        }
    }
}

impl From<RawDependency> for Dependency {
    fn from(value: RawDependency) -> Self {
        unsafe {
            return Self { name: CString::from_raw(value.name.load(Ordering::Relaxed)).to_str().unwrap().to_string() };
        }
    }
}

/// Loads an array from a pointer into a Vec
fn load_array<T: Debug>(ptr: AtomicPtr<RawArray>) -> Vec<T> {
    let ptr = ptr.load(Ordering::Relaxed);
    let len = unsafe { ptr::read(ptr as *mut u64) };
    return load_raw(len, (ptr as u64 + size_of::<u64>() as u64) as *mut T);
}
