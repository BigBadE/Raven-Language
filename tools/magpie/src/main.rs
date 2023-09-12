extern crate alloc;
extern crate core;

use alloc::ffi::CString;
use core::fmt::Debug;
use std::{env, path, ptr};
use std::ffi::{c_char, c_int};
use std::ptr::addr_of;
use std::sync::atomic::{AtomicPtr, Ordering};
use runner::{FileSourceSet, Readable, RunnerSettings, SourceSet};
use syntax::ParsingError;
use crate::arguments::Arguments;
use include_dir::{Dir, DirEntry, File, include_dir};
use tokio::runtime::Builder;

pub mod arguments;

static LIBRARY: Dir = include_dir!("lib/core/src");
static CORE: Dir = include_dir!("tools/magpie/lib/src");

fn main() {
    let base_arguments = Arguments::from_arguments(env::args());
    let build_path = env::current_dir().unwrap().join("build.rv");

    if !build_path.exists() {
        println!("Build file not found!");
        return;
    }

    let arguments = Arguments {
        runner_settings: RunnerSettings {
            io_runtime: base_arguments.runner_settings.io_runtime,
            cpu_runtime: base_arguments.runner_settings.cpu_runtime,
            sources: vec!(Box::new(FileSourceSet {
                root: build_path,
            }), Box::new(InnerSourceSet {
                set: &LIBRARY
            }), Box::new(InnerSourceSet {
                set: &CORE
            })),
            debug: false,
            compiler: "llvm".to_string(),
        },
    };

    println!("Building project...");
    let runner = Builder::new_current_thread().thread_name("main").build().unwrap();
    let value = runner.block_on(run::<AtomicPtr<RawRavenProject>>(&arguments));
    match value {
        Ok(inner) => {
            let raw_project = unsafe { ptr::read(inner.unwrap().load(Ordering::Relaxed)) };
            println!("Processing project... ({:?})", raw_project);
            let project = RavenProject::from(raw_project);
            println!("{:?}", project);
        }
        Err(error) => for error in error {
            println!("{}", error)
        }
    }
}

#[derive(Debug)]
#[repr(C, align(8))]
pub struct RawRavenProject {
    type_id: c_int,
    pub name: AtomicPtr<c_char>,
    pub dependencies: AtomicPtr<RawArray<RawDependency>>,
}

#[derive(Debug)]
pub struct RawArray<T> {
    pub length: i64,
    pub pointer: *const T,
}

unsafe impl<T> Send for RawArray<T> {}

unsafe impl<T> Sync for RawArray<T> {}

#[repr(C, align(8))]
#[derive(Debug)]
pub struct RawDependency {
    type_id: c_int,
    pub name: AtomicPtr<c_char>,
}

#[derive(Debug)]
pub struct RavenProject {
    pub name: String,
    pub dependency: Vec<Dependency>,
}

#[derive(Debug)]
pub struct Dependency {
    pub name: String,
}

fn load_raw<T>(length: i64, pointer: *mut T) -> Vec<T> where T: Debug {
    println!("Pointer: {}", pointer as u64);
    let mut output = Vec::new();
    let temp = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(pointer, length as usize)) };
    for value in temp.into_vec() {
        output.push(value);
    }
    return output;
}

impl From<RawRavenProject> for RavenProject {
    fn from(value: RawRavenProject) -> Self {
        unsafe {
            return Self {
                name: CString::from_raw(value.name.load(Ordering::Relaxed)).to_str().unwrap().to_string(),
                dependency: Vec::from(ptr::read(value.dependencies.load(Ordering::Relaxed))).into_iter()
                    .map(|inner| Dependency::from(inner)).collect::<Vec<_>>(),
            };
        }
    }
}

impl From<RawDependency> for Dependency {
    fn from(value: RawDependency) -> Self {
        unsafe {
            return Self {
                name: CString::from_raw(value.name.load(Ordering::Relaxed)).to_str().unwrap().to_string()
            };
        }
    }
}

impl<T> From<RawArray<T>> for Vec<T> where T: Debug {
    fn from(value: RawArray<T>) -> Self {
        unsafe {
            let len = ptr::read(addr_of!(value.length));
            println!("Loading raw {}", len);
            return load_raw(len, value.pointer as *mut T);
        }
    }
}

async fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    return runner::runner::run::<T>("build::project", &arguments.runner_settings).await;
}

#[derive(Debug)]
pub struct InnerSourceSet {
    set: &'static Dir<'static>,
}

// Forced to make a wrapper due to orphan rule
pub struct FileWrapper {
    file: &'static File<'static>,
}

impl SourceSet for InnerSourceSet {
    fn get_files(&self) -> Vec<Box<dyn Readable>> {
        let mut output = Vec::new();
        read_recursive(&self.set, &mut output);
        return output;
    }

    fn relative(&self, other: &Box<dyn Readable>) -> String {
        let name = other.path()
            .replace(path::MAIN_SEPARATOR, "::");
        return name[0..name.len() - 3].to_string();
    }
}

fn read_recursive(base: &Dir<'static>, output: &mut Vec<Box<dyn Readable>>) {
    for entry in base.entries() {
        match entry {
            DirEntry::Dir(directory) => {
                read_recursive(directory, output);
            }
            DirEntry::File(file) => {
                output.push(Box::new(FileWrapper { file }))
            }
        }
    }
}

impl Readable for FileWrapper {
    fn read(&self) -> String {
        return self.file.contents_utf8().unwrap().to_string();
    }

    fn path(&self) -> String {
        return self.file.path().to_str().unwrap().to_string();
    }
}