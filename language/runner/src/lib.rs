use std::{fs, path};
use std::path::PathBuf;
use anyhow::Error;
use tokio::runtime::Runtime;
use compiler_llvm::LLVMCompiler;
use compilers::compiling::Compiler;

pub mod runner;

pub struct RunnerSettings {
    pub io_runtime: Runtime,
    pub cpu_runtime: Runtime,
    pub sources: Vec<SourceSet>,
    pub debug: bool,
    pub compiler: String
}

impl RunnerSettings {
    pub fn get_compiler(&self) -> Box<dyn Compiler> {
        match self.compiler.to_lowercase().as_str() {
            "llvm" => Box::new(LLVMCompiler::new()),
            _ => panic!("Unknown compilers {}", self.compiler)
        }
    }
}

pub struct SourceSet {
    pub root: PathBuf
}

impl SourceSet {
    pub fn get_files(&self) -> Vec<PathBuf> {
        let mut output = Vec::new();
        SourceSet::read_recursive(&self.root, &mut output)
            .expect(&format!("Failed to read source files! Make sure {:?} exists", self.root));
        return output;
    }

    pub fn relative(&self, other: &PathBuf) -> String {
        let name = other.to_str().unwrap()
            .replace(self.root.to_str().unwrap(), "")
            .replace(path::MAIN_SEPARATOR, "::");
        return name.as_str()[2..name.len()-3].to_string();
    }

    fn read_recursive(base: &PathBuf, output: &mut Vec<PathBuf>) -> Result<(), Error> {
        for file in fs::read_dir(base)? {
            let file = file?;
            if file.file_type()?.is_dir() {
                SourceSet::read_recursive(&file.path(), output)?;
            } else {
                output.push(file.path());
            }
        }
        return Ok(());
    }
}