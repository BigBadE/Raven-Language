use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use std::{env, path};

use ::runner::runner::{build, create_syntax, run};
use include_dir::{include_dir, Dir, DirEntry, File};
use parking_lot::Mutex;

use data::tokens::{Token, TokenTypes};
use data::{Arguments, RavenExtern, Readable, SourceSet};
use parser::tokens::tokenizer::Tokenizer;
use syntax::errors::ParsingError;
use syntax::program::syntax::Syntax;

/// The Raven project types
pub mod project;
mod runner;

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
pub static MAGPIE: Dir = include_dir!("tools/magpie/lib/src");

/// Sets up the arguments with the std
pub fn setup_arguments(arguments: &mut Arguments, source: &mut Vec<Box<dyn SourceSet>>) {
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
}

/// Builds a Raven project, adding the needed dependencies
pub fn build_project<T: RavenExtern + 'static>(
    arguments: &mut Arguments,
    source: &mut Vec<Box<dyn SourceSet>>,
    compile: bool,
) -> Result<(Arc<Mutex<Syntax>>, Option<T>), ()> {
    setup_arguments(arguments, source);
    let value = if compile {
        build_run::<T>(&arguments)
    } else {
        let syntax = create_syntax(arguments);
        arguments.cpu_runtime.block_on(build(syntax.clone(), arguments)).map(|_| (syntax, None))
    };
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
fn build_run<T: RavenExtern + 'static>(arguments: &Arguments) -> Result<(Arc<Mutex<Syntax>>, Option<T>), Vec<ParsingError>> {
    let syntax = create_syntax(arguments);
    let result = arguments.cpu_runtime.block_on(run::<AtomicPtr<T::Input>>(syntax.clone(), arguments))?;
    return Ok((syntax, result.map(|inner| unsafe { RavenExtern::translate(inner.load(Ordering::Relaxed)) })));
}

/// A source set for an internal directory with the include_dir macro
#[derive(Clone, Debug)]
pub struct InnerSourceSet {
    pub set: &'static Dir<'static>,
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
        let mut hasher = DefaultHasher::default();
        Hash::hash(&self.path(), &mut hasher);
        return hasher.finish();
    }
}
