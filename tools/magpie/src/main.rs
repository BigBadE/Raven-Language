use core::fmt::Debug;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{env, path, ptr};

use include_dir::{include_dir, Dir, DirEntry, File};

use data::{
    Arguments, CompilerArguments, FileSourceSet, ParsingError, Readable, RunnerSettings, SourceSet,
};

pub mod project;
mod test;
static CORE: Dir = include_dir!("lib/core/src");
static STD_UNIVERSAL: Dir = include_dir!("lib/std/universal");
static STD_WINDOWS: Dir = include_dir!("lib/std/windows");
static STD_LINUX: Dir = include_dir!("lib/std/linux");
static STD_MACOS: Dir = include_dir!("lib/std/macos");
//static MAGPIE: Dir = include_dir!("tools/magpie/lib/src");

fn main() {
    let args = env::args().collect::<Vec<_>>();

    if args.len() == 2 {
        let target = env::current_dir().unwrap().join(args[1].clone());
        let mut arguments = Arguments::build_args(
            false,
            RunnerSettings {
                sources: vec![],
                debug: false,
                compiler_arguments: CompilerArguments {
                    target: format!(
                        "{}::main",
                        args[1]
                            .clone()
                            .split(path::MAIN_SEPARATOR)
                            .last()
                            .unwrap()
                            .replace(".rv", "")
                    ),
                    compiler: "llvm".to_string(),
                    temp_folder: env::current_dir().unwrap().join("target"),
                },
            },
        );

        println!(
            "Building and running {}...",
            args[1]
                .clone()
                .split(path::MAIN_SEPARATOR)
                .last()
                .unwrap()
                .replace(".rv", "")
        );
        match build::<()>(
            &mut arguments,
            vec![Box::new(FileSourceSet { root: target })],
        ) {
            _ => return,
        }
    } else if args.len() > 2 {
        panic!("Unknown extra arguments! {:?}", args);
    }

    let build_path = env::current_dir().unwrap().join("build.rv");

    if !build_path.exists() {
        println!("Build file not found!");
        return;
    }

    let mut arguments = Arguments::build_args(
        false,
        RunnerSettings {
            sources: vec![],
            debug: false,
            compiler_arguments: CompilerArguments {
                target: "build::project".to_string(),
                compiler: "llvm".to_string(),
                temp_folder: env::current_dir().unwrap().join("target"),
            },
        },
    );

    println!("Setting up build...");
    /*let project = match build::<RawRavenProject>(&mut arguments, vec!(Box::new(FileSourceSet {
        root: build_path,
    }), Box::new(InnerSourceSet {
        set: &MAGPIE
    }))) {
        Ok(found) => match found {
            Some(found) => RavenProject::from(found),
            None => panic!("No project method in build file!")
        },
        Err(()) => panic!("Error during build detected!")
    };*/

    arguments.runner_settings.compiler_arguments.target = "main::main".to_string();

    let source = env::current_dir().unwrap().join("src");

    if !source.exists() {
        panic!("Source folder (src) not found!");
    }

    println!("Building and running project...");
    match build::<()>(
        &mut arguments,
        vec![Box::new(FileSourceSet { root: source })],
    ) {
        _ => {}
    }
}

pub fn build<T: Send + 'static>(
    arguments: &mut Arguments,
    mut source: Vec<Box<dyn SourceSet>>,
) -> Result<Option<T>, ()> {
    let platform_std = match env::consts::OS {
        "windows" => &STD_WINDOWS,
        "linux" => &STD_LINUX,
        "macos" => &STD_MACOS,
        _ => panic!("Unsupported platform {}!", env::consts::OS),
    };

    source.push(Box::new(InnerSourceSet {
        set: &STD_UNIVERSAL,
    }));
    source.push(Box::new(InnerSourceSet { set: platform_std }));
    source.push(Box::new(InnerSourceSet { set: &CORE }));

    arguments.runner_settings.sources = source
        .iter()
        .map(|inner| inner.cloned())
        .collect::<Vec<_>>();

    let value = run::<T>(&arguments);
    return match value {
        Ok(inner) => Ok(inner),
        Err(errors) => {
            println!("Errors:");
            for error in errors {
                error.print(&source);
            }
            Err(())
        }
    };
}

fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    let result = arguments
        .cpu_runtime
        .block_on(runner::runner::run::<AtomicPtr<T>>(&arguments))?;
    return Ok(result.map(|inner| unsafe { ptr::read(inner.load(Ordering::Relaxed)) }));
}

#[derive(Clone, Debug)]
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

    fn relative(&self, other: &dyn Readable) -> String {
        let name = other.path().replace(path::MAIN_SEPARATOR, "::");
        return name[0..name.len() - 3].to_string();
    }

    fn cloned(&self) -> Box<dyn SourceSet> {
        return Box::new(self.clone());
    }
}

fn read_recursive(base: &Dir<'static>, output: &mut Vec<Box<dyn Readable>>) {
    for entry in base.entries() {
        match entry {
            DirEntry::Dir(directory) => {
                read_recursive(directory, output);
            }
            DirEntry::File(file) => output.push(Box::new(FileWrapper { file })),
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
