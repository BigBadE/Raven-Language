extern crate alloc;
extern crate core;

use alloc::ffi::CString;
use core::fmt::Debug;
use std::{env, path, ptr};
use std::ffi::{c_char, c_int};
use std::mem::size_of;
use std::sync::atomic::{AtomicPtr, Ordering};
use data::{FileSourceSet, Readable, RunnerSettings, SourceSet, ParsingError, Arguments};
use include_dir::{Dir, DirEntry, File, include_dir};

static CORE: Dir = include_dir!("lib/core/src");
static STD_UNIVERSAL: Dir = include_dir!("lib/std/universal");
static STD_WINDOWS: Dir = include_dir!("lib/std/windows");
static STD_LINUX: Dir = include_dir!("lib/std/linux");
static STD_MACOS: Dir = include_dir!("lib/std/macos");
static MAGPIE: Dir = include_dir!("tools/magpie/lib/src");

fn main() {
    let build_path = env::current_dir().unwrap().join("build.rv");

    if !build_path.exists() {
        println!("Build file not found!");
        return;
    }

    let mut arguments = Arguments::build_args(false, RunnerSettings {
        sources: vec!(Box::new(FileSourceSet {
            root: build_path,
        }), Box::new(InnerSourceSet {
            set: &MAGPIE
        }), Box::new(InnerSourceSet {
            set: &CORE
        })),
        debug: false,
        compiler: "llvm".to_string(),
    });

    println!("Building project...");
    let value = run::<RawRavenProject>(&arguments);
    let project = match value {
        Ok(inner) => match inner {
            Some(found) => RavenProject::from(found),
            None => panic!("No project method in build file!")
        },
        Err(error) => panic!("{:?}", error)
    };

    let source = env::current_dir().unwrap().join("src");

    if !source.exists() {
        println!("Build file not found!");
        return;
    }

    let platform_std = match env::consts::OS {
        "windows" => &STD_WINDOWS,
        "linux" => &STD_LINUX,
        "macos" => &STD_MACOS,
        _ => panic!("Unsupported platform {}!", env::consts::OS)
    };

    arguments.runner_settings.sources = vec!(Box::new(FileSourceSet {
        root: source,
    }), Box::new(InnerSourceSet {
        set: &STD_UNIVERSAL
    }), Box::new(InnerSourceSet {
        set: platform_std
    }), Box::new(InnerSourceSet {
        set: &CORE
    }));
}

#[derive(Debug)]
#[repr(C, align(8))]
pub struct RawRavenProject {
    type_id: c_int,
    pub name: AtomicPtr<c_char>,
    pub dependencies: AtomicPtr<RawArray>,
}

#[derive(Debug)]
pub struct RawArray {}

#[repr(C, align(8))]
#[derive(Debug)]
pub struct RawDependency {
    type_id: c_int,
    pub name: AtomicPtr<c_char>,
}

#[derive(Debug)]
pub struct RavenProject {
    pub name: String,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug)]
pub struct Dependency {
    //pub name: String,
}

fn load_raw<T>(length: u64, pointer: *mut T) -> Vec<T> {
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
                dependencies: load_array(value.dependencies).into_iter()
                    .map(|inner: RawDependency| Dependency::from(inner)).collect::<Vec<_>>(),
            };
        }
    }
}

impl From<RawDependency> for Dependency {
    fn from(value: RawDependency) -> Self {
        unsafe {
            return Self {
                //name: CString::from_raw(value.name.load(Ordering::Relaxed)).to_str().unwrap().to_string()
            };
        }
    }
}

fn load_array<T>(ptr: AtomicPtr<RawArray>) -> Vec<T> {
    let ptr = ptr.load(Ordering::Relaxed);
    let len = unsafe { ptr::read(ptr as *mut u64) };
    return load_raw(len, (ptr as u64 + size_of::<u64>() as u64) as *mut T);
}

fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    let result = arguments.cpu_runtime.block_on(
        runner::runner::run::<AtomicPtr<T>>("build::project".to_string(), &arguments))?;
    return Ok(result.map(|inner| unsafe { ptr::read(inner.load(Ordering::Relaxed)) }));
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