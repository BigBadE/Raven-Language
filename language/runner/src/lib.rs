use std::{fs, path};
use std::collections::HashMap;
use std::fmt::Debug;
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
    pub sources: Vec<Box<dyn SourceSet>>,
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

pub trait Readable {
    fn read(&self) -> String;

    fn path(&self) -> String;
}

pub trait SourceSet: Debug {
    fn get_files(&self) -> Vec<Box<dyn Readable>>;

    fn relative(&self, other: &Box<dyn Readable>) -> String;
}

#[derive(Debug)]
pub struct FileSourceSet {
    pub root: PathBuf,
}

impl Readable for PathBuf {
    fn read(&self) -> String {
        return fs::read_to_string(self.clone()).expect(
            &format!("Failed to read source file: {}", self.to_str().unwrap()));
    }

    fn path(&self) -> String {
        return self.to_str().unwrap().to_string();
    }
}

impl SourceSet for FileSourceSet {
    fn get_files(&self) -> Vec<Box<dyn Readable>> {
        let mut output = Vec::new();
        read_recursive(self.root.clone(), &mut output)
            .expect(&format!("Failed to read source files! Make sure {:?} exists", self.root));
        return output;
    }

    fn relative(&self, other: &Box<dyn Readable>) -> String {
        let name = other.path()
            .replace(self.root.to_str().unwrap(), "")
            .replace(path::MAIN_SEPARATOR, "::");
        if name.len() == 0 {
            let path = other.path();
            let name: &str = path.split(path::MAIN_SEPARATOR).last().unwrap();
            return name[0..name.len()-3].to_string();
        }
        return name.as_str()[2..name.len() - 3].to_string();
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