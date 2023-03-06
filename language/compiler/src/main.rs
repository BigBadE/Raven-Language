#![feature(get_mut_unchecked)]

use std::env;
use std::path::PathBuf;
use inkwell::context::Context;
use crate::compiler::Compiler;
use crate::file::FileStructureImpl;
use crate::types::type_resolver::CompilerTypeResolver;

pub mod instructions;

pub mod compiler;
pub mod file;
pub mod function_compiler;
pub mod types;
pub mod util;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let directory = FileStructureImpl::new(PathBuf::from(args.get(1).unwrap()));

    let context = Context::create();
    let mut type_manager = CompilerTypeResolver::new(&context);
    parser::parse(&mut type_manager, Box::new(directory));
    let mut compiler = Compiler::new(type_manager, &context);
    match compiler.compile() {
        Some(main) => {
            unsafe { println!("Output: {}", main.call()) }
        },
        None => panic!("Couldn't find main!")
    };
}