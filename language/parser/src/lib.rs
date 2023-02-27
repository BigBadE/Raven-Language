#[macro_use]
extern crate pest_derive;
extern crate core;

use std::fs;
use std::path::PathBuf;
use ast::program::Program;
use ast::TopElement;

pub mod code;
pub mod function;
pub mod parser;

pub fn parse(input: Box<dyn FileStructure>) -> Program {
    let mut output = Program::new();
    let root_offset = input.get_root().to_str().unwrap().len() + 1;
    for file in input.get_files() {
        let name = file.to_str().unwrap()[root_offset..file.to_str().unwrap().len() - 3].to_string();
        let elements = parser::parse(&name, fs::read_to_string(&file).unwrap());
        for element in elements {
            println!("{}", element);
            match element {
                TopElement::Function(function) => {
                    output.static_functions.insert(function.name.value.clone(), function);
                }
                TopElement::Struct(structure) => {
                    output.elem_types.insert(structure.name.value.clone(), structure);
                }
            }
        }
    }
    return output;
}

pub trait FileStructure {
    fn get_files(&self) -> Vec<PathBuf>;

    fn get_root(&self) -> PathBuf;
}