use anyhow::Error;
use colored::Colorize;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;
use std::{fs, path};
use tokio::runtime::{Builder, Runtime};

pub type Main<T> = unsafe extern "C" fn() -> T;

pub struct RunnerSettings {
    pub sources: Vec<Box<dyn SourceSet>>,
    pub debug: bool,
    pub compiler_arguments: CompilerArguments,
}

#[derive(Clone)]
pub struct CompilerArguments {
    pub compiler: String,
    pub target: String,
    pub temp_folder: PathBuf,
}

pub struct Arguments {
    pub io_runtime: Option<Runtime>,
    pub cpu_runtime: Runtime,
    pub runner_settings: RunnerSettings,
}

impl Arguments {
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
                Some(
                    io_runtime
                        .enable_time()
                        .thread_name("io-runtime")
                        .build()
                        .expect("Failed to build I/O runtime"),
                )
            },
            cpu_runtime: cpu_runtime
                .enable_time()
                .thread_name("cpu-runtime")
                .build()
                .expect("Failed to build CPU runtime"),
            runner_settings,
        };
    }
}

impl RunnerSettings {
    pub fn include_references(&self) -> bool {
        return match self.compiler_arguments.compiler.to_lowercase().as_str() {
            "llvm" => true,
            _ => panic!("Unknown compiler {}", self.compiler_arguments.compiler),
        };
    }
}

pub trait Readable {
    fn read(&self) -> String;

    fn path(&self) -> String;
}

pub trait SourceSet: Debug + Send + Sync {
    fn get_files(&self) -> Vec<Box<dyn Readable>>;

    fn relative(&self, other: &dyn Readable) -> String;

    fn cloned(&self) -> Box<dyn SourceSet>;
}

#[derive(Clone, Debug)]
pub struct FileSourceSet {
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
        read_recursive(self.root.clone(), &mut output).unwrap_or_else(|_| {
            panic!(
                "Failed to read source files! Make sure {:?} exists",
                self.root
            )
        });
        return output;
    }

    fn relative(&self, other: &dyn Readable) -> String {
        let name = other
            .path()
            .replace(self.root.to_str().unwrap(), "")
            .replace(path::MAIN_SEPARATOR, "::");
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

// An error somewhere in a source file, with exact location.
#[derive(Clone, Debug)]
pub struct ParsingError {
    // Name of the file this error is in
    pub file: String,
    // The line number and index from that line
    pub start: (u32, u32),
    //
    pub start_offset: usize,
    pub end: (u32, u32),
    pub end_offset: usize,
    pub message: String,
}

impl ParsingError {
    // An empty error, used for places where errors are ignored
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

    pub fn new(
        file: String,
        start: (u32, u32),
        start_offset: usize,
        end: (u32, u32),
        end_offset: usize,
        message: String,
    ) -> Self {
        return Self {
            file,
            start,
            start_offset,
            end,
            end_offset,
            message,
        };
    }

    pub fn print(&self, sources: &Vec<Box<dyn SourceSet>>) {
        let mut file = None;
        'outer: for source in sources {
            for readable in source.get_files() {
                if source.relative(&*readable) == self.file {
                    file = Some(readable);
                    break 'outer;
                }
            }
        }

        if file.is_none() {
            panic!("Missing file: {}", self.message);
        }
        let file = file.unwrap();
        let contents = file.read();
        let line = contents
            .lines()
            .nth((self.start.0 as usize).max(1) - 1)
            .unwrap_or("???");
        println!("{}", self.message.bright_red());
        println!(
            "{}",
            format!("in file {}:{}:{}", file.path(), self.start.0, self.start.1).bright_red()
        );
        println!(
            "{} {}",
            " ".repeat(self.start.0.to_string().len()),
            "|".bright_cyan()
        );
        println!(
            "{} {} {}",
            self.start.0.to_string().bright_cyan(),
            "|".bright_cyan(),
            line.bright_red()
        );
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
        return write!(
            f,
            "Error at {} ({}:{}):\n{}",
            self.file, self.start.0, self.start.1, self.message
        );
    }
}
