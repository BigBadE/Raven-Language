use std::fs;
use std::path::PathBuf;
use ast::compiler::CompilerInfo;
use ast::program::Program;
use ast::type_resolver::TypeResolver;
use crate::parser::ParseError;

pub mod code;
pub mod literal;
pub mod parser;
pub mod top_elements;
pub mod util;

pub fn parse<'a>(compiler_info: &mut dyn CompilerInfo<'a>, type_manager: &mut dyn TypeResolver<'a>, input: Box<dyn FileStructure>) -> Program<'a> {
    let mut output = Program::new();
    let root_offset = input.get_root().to_str().unwrap().len() + 1;

    for file in input.get_files() {
        let name = file.to_str().unwrap()[root_offset..file.to_str().unwrap().len() - 3].to_string();
        match parser::parse(&mut output, type_manager, &name, fs::read_to_string(&file).unwrap(), true) {
            Ok(_) => {},
            Err(errors) => print_errors(errors)
        };
    }

    for (_name, static_function) in &mut output.static_functions {
        static_function.finalize(type_manager);
    }

    compiler_info.finalize_types(type_manager);

    for file in input.get_files() {
        let name = file.to_str().unwrap()[root_offset..file.to_str().unwrap().len() - 3].to_string();
        match parser::parse(&mut output, type_manager, &name, fs::read_to_string(&file).unwrap(), false) {
            Ok(_) => {},
            Err(errors) => print_errors(errors)
        };
    }

    for (_name, structure) in type_manager.get_types() {
        println!("{}", structure);
    }

    for (_name, function) in &output.static_functions {
        println!("{}", function);
    }

    return output;
}

fn print_errors(failed: Vec<ParseError>) {
    if !failed.is_empty() {
        let mut errors = "Parsing Errors:\n".to_string();
        for error in failed {
            errors += format!("\n{}\n", error).as_str();
        }
        panic!("{}", errors);
    }
}

pub trait FileStructure {
    fn get_files(&self) -> Vec<PathBuf>;

    fn get_root(&self) -> PathBuf;
}