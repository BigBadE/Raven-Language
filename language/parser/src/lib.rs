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
    let root_offset = input.get_root().to_str().unwrap().len()+1;
    for file in input.get_files() {
        let name = file.to_str().unwrap()[root_offset..file.to_str().unwrap().len()-3].to_string();
        let elements = parser::parse(&name, fs::read_to_string(&file).unwrap());
        for element in elements {
            match element {
                TopElement::Function(function) => {
                    let name = function.name.value.split("::").last().unwrap();
                    if output.static_functions.contains_key(name) {
                        let other = output.static_functions.remove(name).unwrap();
                        if other.name.value == function.name.value {
                            panic!("Duplicate name: {}", other.name);
                        }
                        output.static_functions.insert(other.name.value.clone(), other);
                        let name = match &output.package_name {
                            Some(package) => package.clone() + "::" + function.name.value.as_str(),
                            None => function.name.value.clone()
                        };
                        if function.name.value == "main::main" {
                            output.set_main(name.to_string());
                        }
                        println!("{}", function);
                        output.static_functions.insert(name, function);
                    } else {
                        if function.name.value == "main::main" {
                            output.set_main(name.to_string());
                        }
                        println!("{}", function);
                        output.static_functions.insert(name.to_string(), function);
                    }
                },
                TopElement::Struct(structure) => {
                    let name = structure.name.value.split("::").last().unwrap();
                    if output.elem_types.contains_key(name) {
                        let other = output.elem_types.remove(name).unwrap();
                        if other.name.value == structure.name.value {
                            panic!("Duplicate name: {}", other.name);
                        }
                        output.elem_types.insert(other.name.value.clone(), other);
                        let name = match &output.package_name {
                            Some(package) => package.clone() + "::" + structure.name.value.as_str(),
                            None => structure.name.value.clone()
                        };
                        println!("{}", structure);
                        output.elem_types.insert(name, structure);
                    } else {
                        println!("{}", structure);
                        output.elem_types.insert(name.to_string(), structure);
                    }
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