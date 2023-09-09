use std::{env, path, ptr};
use std::os::raw::c_int;
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
    let value = runner.block_on(run::<AtomicPtr<Test>>(&arguments));
    match value {
        Ok(inner) => {
            let value = unsafe { ptr::read(inner.unwrap().load(Ordering::Relaxed)) };
            println!("Value: {} and {}", value.inner, unsafe { ptr::read(value.other.load(Ordering::Relaxed))});
        },
        Err(error) => for error in error {
            println!("{}", error)
        }
    }
}

#[repr(C, align(8))]
#[derive(Debug)]
pub struct Test {
    pub inner: c_int,
    pub other: AtomicPtr<c_int>
}

async fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    return runner::runner::run::<T>("build::project", &arguments.runner_settings).await;
}

#[derive(Debug)]
pub struct InnerSourceSet {
    set: &'static Dir<'static>
}

// Forced to make a wrapper due to orphan rule
pub struct FileWrapper {
    file: &'static File<'static>
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
        return name[0..name.len()-3].to_string();
    }
}

fn read_recursive(base: &Dir<'static>, output: &mut Vec<Box<dyn Readable>>) {
    for entry in base.entries() {
        match entry {
            DirEntry::Dir(directory) => {
                read_recursive(directory, output);
            },
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