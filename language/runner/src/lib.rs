use std::{fs, path};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Error;
use tokio::runtime::Runtime;
use compiler_llvm::LLVMCompiler;
use syntax::function::FinalizedFunction;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::Compiler;

pub mod runner;

pub struct RunnerSettings {
    pub io_runtime: Runtime,
    pub cpu_runtime: Runtime,
    pub sources: Vec<SourceSet>,
    pub debug: bool,
    pub compiler: String,
}

pub fn get_compiler<T>(compiling: Arc<HashMap<String, Arc<FinalizedFunction>>>, struct_compiling: Arc<HashMap<String, Arc<FinalizedStruct>>>,
                       name: String) -> Box<dyn Compiler<T>> {
    return Box::new(match name.to_lowercase().as_str() {
        "llvm" => LLVMCompiler::new(compiling, struct_compiling),
        _ => panic!("Unknown compilers {}", name)
    });
}

impl RunnerSettings {
    pub fn include_references(&self) -> bool {
        return match self.compiler.to_lowercase().as_str() {
            "llvm" => true,
            _ => panic!("Unknown compiler {}", self.compiler)
        };
    }
}

#[derive(Debug)]
pub struct SourceSet {
    pub root: PathBuf,
}

impl SourceSet {
    pub fn get_files(&self) -> Vec<PathBuf> {
        let mut output = Vec::new();
        SourceSet::read_recursive(self.root.clone(), &mut output)
            .expect(&format!("Failed to read source files! Make sure {:?} exists", self.root));
        return output;
    }

    pub fn relative(&self, other: &PathBuf) -> String {
        let name = other.to_str().unwrap()
            .replace(self.root.to_str().unwrap(), "")
            .replace(path::MAIN_SEPARATOR, "::");
        return name.as_str()[2..name.len() - 3].to_string();
    }

    fn read_recursive(base: PathBuf, output: &mut Vec<PathBuf>) -> Result<(), Error> {
        if fs::metadata(&base)?.file_type().is_dir() {
            for file in fs::read_dir(&base)? {
                let file = file?;
                SourceSet::read_recursive(file.path(), output)?;
            }
        } else {
            output.push(base);
        }
        return Ok(());
    }
}