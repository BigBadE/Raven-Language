use std::env;
use std::path::PathBuf;
use inkwell::context::Context;
use crate::types::type_manager::TypeManager;
use crate::compiler::Compiler;
use crate::file::FileStructureImpl;

pub mod instructions;

pub mod compiler;
pub mod file;
pub mod function_compiler;
pub mod types;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let directory = FileStructureImpl::new(PathBuf::from(args.get(1).unwrap()));

    let context = Context::create();
    let types = TypeManager::new(&context);
    match Compiler::new(&context, &types).compile(directory) {
        Some(main) => unsafe { println!("Output: {}", main.call()) },
        None => panic!("Couldn't find main!")
    };
}