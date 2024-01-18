use core::fmt::Debug;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{env, path};

use include_dir::{include_dir, Dir, DirEntry, File};

use data::tokens::{Token, TokenTypes};
use data::{Arguments, CompilerArguments, ParsingError, RavenExtern, Readable, RunnerSettings, SourceSet};
use parser::tokens::tokenizer::Tokenizer;
use parser::FileSourceSet;

use crate::project::RavenProject;

/// The Raven project types
pub mod project;
mod test;

/// The core Raven library
static CORE: Dir = include_dir!("lib/core/src");
/// The universal standard library, which calls per-platform standards
static STD_UNIVERSAL: Dir = include_dir!("lib/std/universal/src");
/// The windows standard library
static STD_WINDOWS: Dir = include_dir!("lib/std/windows/src");
/// The linux standard library
static STD_LINUX: Dir = include_dir!("lib/std/linux/src");
/// The MacOS standard library
static STD_MACOS: Dir = include_dir!("lib/std/macos/src");
/// The Magpie classes
static MAGPIE: Dir = include_dir!("tools/magpie/lib/src");

/// Finds the Raven project/file and runs it
fn main() {
    let args = env::args().collect::<Vec<_>>();

    if args.len() == 2 {
        let target = env::current_dir().unwrap().join(args[1].clone());
        let mut arguments = Arguments::build_args(
            false,
            RunnerSettings {
                sources: vec![],
                compiler_arguments: CompilerArguments {
                    target: format!(
                        "{}::main",
                        args[1].clone().split(path::MAIN_SEPARATOR).last().unwrap().replace(".rv", "")
                    ),
                    compiler: "llvm".to_string(),
                    temp_folder: env::current_dir().unwrap().join("target"),
                },
            },
        );

        println!(
            "Building and running {}...",
            args[1].clone().split(path::MAIN_SEPARATOR).last().unwrap().replace(".rv", "")
        );
        match build::<()>(&mut arguments, vec![Box::new(FileSourceSet { root: target })]) {
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
            compiler_arguments: CompilerArguments {
                target: "build::project".to_string(),
                compiler: "llvm".to_string(),
                temp_folder: env::current_dir().unwrap().join("target"),
            },
        },
    );

    println!("Setting up build...");
    let project = match build::<RavenProject>(
        &mut arguments,
        vec![Box::new(FileSourceSet { root: build_path }), Box::new(InnerSourceSet { set: &MAGPIE })],
    ) {
        Ok(found) => match found {
            Some(found) => RavenProject::from(found),
            None => {
                println!("No project method in build file!");
                return;
            }
        },
        Err(()) => return,
    };

    arguments.runner_settings.compiler_arguments.target = "main::main".to_string();

    let source = env::current_dir().unwrap().join("src");

    if !source.exists() {
        panic!("Source folder (src) not found!");
    }

    println!("Building and running {}...", project.name);
    match build::<()>(&mut arguments, vec![Box::new(FileSourceSet { root: source })]) {
        _ => {}
    }
}

/// Builds a Raven project, adding the needed dependencies
pub fn build<T: RavenExtern + 'static>(
    arguments: &mut Arguments,
    mut source: Vec<Box<dyn SourceSet>>,
) -> Result<Option<T>, ()> {
    let platform_std = match env::consts::OS {
        "windows" => &STD_WINDOWS,
        "linux" => &STD_LINUX,
        "macos" => &STD_MACOS,
        _ => panic!("Unsupported platform {}!", env::consts::OS),
    };

    source.push(Box::new(InnerSourceSet { set: &STD_UNIVERSAL }));
    source.push(Box::new(InnerSourceSet { set: platform_std }));
    source.push(Box::new(InnerSourceSet { set: &CORE }));

    arguments.runner_settings.sources = source.iter().map(|inner| inner.cloned()).collect::<Vec<_>>();

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

/// Runs Raven and blocks until a result is gotten
fn run<T: RavenExtern + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    let result = arguments.cpu_runtime.block_on(runner::runner::run::<AtomicPtr<T::Input>>(&arguments))?;
    return Ok(result.map(|inner| unsafe { RavenExtern::translate(inner.load(Ordering::Relaxed)) }));
}

/// A source set for an internal directory with the include_dir macro
#[derive(Clone, Debug)]
pub struct InnerSourceSet {
    set: &'static Dir<'static>,
}

/// Forced to make a wrapper to implement Readable due to orphan rule
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
        let name = other.path().replace(path::MAIN_SEPARATOR, "::").replace('/', "::");
        return name[0..name.len() - 3].to_string();
    }

    fn cloned(&self) -> Box<dyn SourceSet> {
        return Box::new(self.clone());
    }
}

/// Recursively reads an include_dir directory to the output
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
    fn read(&self) -> Vec<Token> {
        let binding = self.contents();
        let mut tokenizer = Tokenizer::new(binding.as_bytes());
        let mut tokens = Vec::default();
        loop {
            tokens.push(tokenizer.next());
            if tokens.last().unwrap().token_type == TokenTypes::EOF {
                break;
            }
        }

        return tokens;
    }

    fn contents(&self) -> String {
        return self.file.contents_utf8().unwrap().to_string();
    }

    fn path(&self) -> String {
        return self.file.path().to_str().unwrap().to_string();
    }

    fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&self.path(), &mut hasher);
        return hasher.finish();
    }
}
