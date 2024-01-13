#![feature(try_trait_v2)]
use anyhow::Error;
use colored::Colorize;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;
use std::{fs, path};
use tokio::runtime::{Builder, Runtime};

/// The type of the main LLVM function called by the program
pub type Main<T> = unsafe extern "C" fn() -> T;

pub mod externs;
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
pub trait Readable {
    /// Reads the readable to a string
    fn read(&self) -> String;

    /// Gets the path of the readable
    fn path(&self) -> String;
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

/// A simple source set of a single file/folder
#[derive(Clone, Debug)]
pub struct FileSourceSet {
    /// The path of the file/folder
    pub root: PathBuf,
}

impl Readable for PathBuf {
    fn read(&self) -> String {
        return fs::read_to_string(self.clone())
            .unwrap_or_else(|_| panic!("Failed to read source file: {}", self.to_str().unwrap()));
    }

    fn path(&self) -> String {
        return self.to_str().unwrap().to_string();
    }
}

impl SourceSet for FileSourceSet {
    fn get_files(&self) -> Vec<Box<dyn Readable>> {
        let mut output = Vec::default();
        read_recursive(self.root.clone(), &mut output)
            .unwrap_or_else(|_| panic!("Failed to read source files! Make sure {:?} exists", self.root));
        return output;
    }

    fn relative(&self, other: &dyn Readable) -> String {
        let name =
            other.path().replace(self.root.to_str().unwrap(), "").replace(path::MAIN_SEPARATOR, "::").replace('/', "::");
        if name.len() == 0 {
            let path = other.path();
            let name: &str = path.split(path::MAIN_SEPARATOR).last().unwrap();
            return name[0..name.len() - 3].to_string();
        }
        return name.as_str()[2..name.len() - 3].to_string();
    }

    fn cloned(&self) -> Box<dyn SourceSet> {
        return Box::new(self.clone());
    }
}

/// Recursively reads a folder/file into the list of files
fn read_recursive(base: PathBuf, output: &mut Vec<Box<dyn Readable>>) -> Result<(), Error> {
    if fs::metadata(&base)?.file_type().is_dir() {
        for file in fs::read_dir(&base)? {
            let file = file?;
            read_recursive(file.path(), output)?;
        }
    } else {
        output.push(Box::new(base));
    }
    return Ok(());
}

/// An error somewhere in a source file, with exact location.
#[derive(Clone, Debug)]
pub struct ParsingError {
    /// Name of the file this error is in
    pub file: String,
    /// The line number and index from that line
    pub start: (u32, u32),
    /// Offset from the start of the file
    pub start_offset: usize,
    /// The line number and index from that line
    pub end: (u32, u32),
    /// Offset from the start of the file
    pub end_offset: usize,
    /// The error message
    pub message: String,
}

impl ParsingError {
    /// An empty error, used for places where errors are ignored
    pub fn empty() -> Self {
        return ParsingError {
            file: String::default(),
            start: (0, 0),
            start_offset: 0,
            end: (0, 0),
            end_offset: 0,
            message: "You shouldn't see this! Report this please!".to_string(),
        };
    }

    /// Creates a new error
    pub fn new(
        file: String,
        start: (u32, u32),
        start_offset: usize,
        end: (u32, u32),
        end_offset: usize,
        message: String,
    ) -> Self {
        return Self { file, start, start_offset, end, end_offset, message };
    }

    /// Prints the error to console
    pub fn print(&self, sources: &Vec<Box<dyn SourceSet>>) {
        let mut file = None;
        'outer: for source in sources {
            for readable in source.get_files() {
                if self.file.starts_with(&source.relative(&*readable)) {
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
        let contents = file.read();
        let line = contents.lines().nth((self.start.0 as usize).max(1) - 1).unwrap_or("???");
        println!("{}", self.message.bright_red());
        println!("{}", format!("in file {}:{}:{}", file.path(), self.start.0, self.start.1).bright_red());
        println!("{} {}", " ".repeat(self.start.0.to_string().len()), "|".bright_cyan());
        println!("{} {} {}", self.start.0.to_string().bright_cyan(), "|".bright_cyan(), line.bright_red());
        println!(
            "{} {} {}{}",
            " ".repeat(self.start.0.to_string().len()),
            "|".bright_cyan(),
            " ".repeat(self.start.1 as usize),
            "^".repeat(self.end_offset - self.start_offset).bright_red()
        );
    }
}

impl Display for ParsingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "Error at {} ({}:{}):\n{}", self.file, self.start.0, self.start.1, self.message);
    }
}

/// A small type for translating external Raven types into Rust types
pub trait RavenExtern {
    type Input;
    unsafe fn translate(raven_type: *mut Self::Input) -> Self;
}
