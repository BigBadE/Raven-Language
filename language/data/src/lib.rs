#![feature(try_trait_v2)]

use std::fmt::Debug;
use std::path::PathBuf;

use colored::Colorize;
use tokio::runtime::{Builder, Runtime};

use crate::tokens::{Span, Token};

/// The type of the main LLVM function called by the program
pub type Main<T> = unsafe extern "C" fn() -> T;

/// Handles the externals for translating Raven types to Rust types
pub mod externs;
/// Tokens
pub mod tokens;

/// Settings used in configuring the runner
pub struct RunnerSettings {
    /// Sources to pull source raven files from
    pub sources: Vec<Box<dyn SourceSet>>,
    /// Arguments for the compiler
    pub compiler_arguments: CompilerArguments,
}

/// Arguments used when configuring the compiler
#[derive(Clone)]
pub struct CompilerArguments {
    /// Which compiler to use, defaults to LLVM
    pub compiler: String,
    /// Target method to return as the main method, usually main::main or (name)::test
    pub target: String,
    /// The temp folder to use while compiling
    pub temp_folder: PathBuf,
}

/// Arguments for running Raven
pub struct Arguments {
    /// The IO runtime, defaults to cpu_runtime if None. Can be set to None in single-threaded environments
    pub io_runtime: Option<Runtime>,
    /// The CPU runtime, which is used for CPU-intense tasks
    pub cpu_runtime: Runtime,
    /// The settings for the runner running Raven
    pub runner_settings: RunnerSettings,
}

impl Arguments {
    /// Builds the arguments with the runner settings
    pub fn build_args(single_threaded: bool, runner_settings: RunnerSettings) -> Arguments {
        let (mut io_runtime, mut cpu_runtime) = if single_threaded {
            (Builder::new_current_thread(), Builder::new_current_thread())
        } else {
            (Builder::new_multi_thread(), Builder::new_multi_thread())
        };

        return Arguments {
            io_runtime: if single_threaded {
                None
            } else {
                Some(io_runtime.enable_time().thread_name("io-runtime").build().expect("Failed to build I/O runtime"))
            },
            cpu_runtime: cpu_runtime.enable_time().thread_name("cpu-runtime").build().expect("Failed to build CPU runtime"),
            runner_settings,
        };
    }
}

impl RunnerSettings {
    /// Whether to include references, LLVM requires it but runtimes like the JVM doesn't
    pub fn include_references(&self) -> bool {
        return match self.compiler_arguments.compiler.to_lowercase().as_str() {
            "llvm" => true,
            _ => panic!("Unknown compiler {}", self.compiler_arguments.compiler),
        };
    }
}

/// A readable type
pub trait Readable: Send {
    /// Reads the readable to a list of tokens
    fn read(&self) -> Vec<Token>;

    /// Gets the file's contents
    fn contents(&self) -> String;

    /// Gets the path of the readable
    fn path(&self) -> String;

    fn hash(&self) -> u64;
}

/// A set of Raven sources
pub trait SourceSet: Debug + Send + Sync {
    /// Returns all of the contained sources
    fn get_files(&self) -> Vec<Box<dyn Readable>>;

    /// Gets the relative path in folder/file format, with no extension
    fn relative(&self, other: &dyn Readable) -> String;

    /// Clones the source set and boxes it
    fn cloned(&self) -> Box<dyn SourceSet>;
}

/// An error somewhere in a source file, with exact location.
#[derive(Clone, Debug)]
pub struct ParsingError {
    /// The location of the error
    pub span: Span,
    /// The error message
    pub message: &'static str,
}

impl ParsingError {
    /// Creates a new error
    pub fn new(span: Span, message: &'static str) -> Self {
        return Self { span, message };
    }

    /// Prints the error to console
    pub fn print(&self, sources: &Vec<Box<dyn SourceSet>>) {
        let mut file = None;
        'outer: for source in sources {
            for readable in source.get_files() {
                if self.span.file == readable.hash() {
                    file = Some(readable);
                    break 'outer;
                }
            }
        }

        if file.is_none() {
            println!("Missing file: {}", self.message);
            return;
        }
        let file = file.unwrap();
        let contents = file.contents();
        let tokens = file.read();
        let mut token = tokens[self.span.start].clone();
        if self.span.start != self.span.end {
            let end = &tokens[self.span.end];
            token.end = end.end;
            token.end_offset = end.end_offset;
        }

        // Multi-line tokens aren't supported, set the end to the start
        if token.start.0 != token.end.0 {
            token.start_offset = token.end_offset - token.end.1 as usize;
            token.start = (token.end.0, 0);
        }

        if token.end_offset == token.start_offset {
            token.start_offset -= 1;
        }

        let line = contents.lines().nth((token.start.0 as usize).max(1) - 1).unwrap_or("???");
        println!("{}", self.message.bright_red());
        println!("{}", format!("in file {}:{}:{}", file.path(), token.start.0, token.start.1).bright_red());
        println!("{} {}", " ".repeat(token.start.0.to_string().len()), "|".bright_cyan());
        println!("{} {} {}", token.start.0.to_string().bright_cyan(), "|".bright_cyan(), line.bright_red());
        println!(
            "{} {} {}{}",
            " ".repeat(token.start.0.to_string().len()),
            "|".bright_cyan(),
            " ".repeat(token.start.1 as usize),
            "^".repeat(token.end_offset - token.start_offset).bright_red()
        );
    }
}

/// A small type for translating external Raven types into Rust types
pub trait RavenExtern {
    type Input;
    unsafe fn translate(raven_type: *mut Self::Input) -> Self;
}
