extern crate core;

use std::fs;
use std::path::PathBuf;
use ast::type_resolver::TypeResolver;
use crate::parser::ParseError;

pub mod code;
pub mod conditional;
pub mod literal;
pub mod parser;
pub mod top_elements;
pub mod util;

pub fn parse(type_manager: &mut dyn TypeResolver, input: Box<dyn FileStructure>) {
    let root_offset = input.get_root().to_str().unwrap().len() + 1;

    for file in input.get_files() {
        let name = file.to_str().unwrap()[root_offset..file.to_str().unwrap().len() - 3].to_string();
        match parser::parse(type_manager, &name, fs::read_to_string(&file).unwrap()) {
            Ok(_) => {},
            Err(errors) => print_errors(errors)
        };
    }
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