use std::env;
use std::path::PathBuf;
use inkwell::context::Context;
use crate::compiler::Compiler;
use crate::file::FileStructureImpl;

pub mod compiler;
pub mod context;
pub mod file;
pub mod function_compiler;
pub mod types;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let directory = FileStructureImpl::new(PathBuf::from(args.get(1).unwrap()));

    match Compiler::new().compile(directory) {
        Some(main) => unsafe { main.call() },
        None => panic!("Couldn't find main!")
    };
}