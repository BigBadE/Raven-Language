use std::{env, path};
use tokio::main;
use runner::{FileSourceSet, Readable, RunnerSettings, SourceSet};
use syntax::ParsingError;
use crate::arguments::Arguments;
use include_dir::{Dir, DirEntry, File, include_dir};

pub mod arguments;

static LIBRARY: Dir = include_dir!("lib/core/src");
static CORE: Dir = include_dir!("tools/magpie/lib/src");

#[main]
async fn main() {
    let base_arguments = Arguments::from_arguments(env::args());
    let build_path = env::current_dir().unwrap().join("build.rv");

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
    let value = run::<i64>(&arguments).await;
}

async fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    return runner::runner::run::<T>("build::project", &arguments.runner_settings).await;
}

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