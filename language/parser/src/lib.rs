use std::fs;
use std::path::PathBuf;
use ast::program::Program;
use ast::TopElement;

pub mod code;
pub mod parser;
pub mod top_elements;
pub mod util;

pub fn parse(input: Box<dyn FileStructure>) -> Program {
    let mut output = Program::new();
    let root_offset = input.get_root().to_str().unwrap().len() + 1;
    let mut failed = Vec::new();

    for file in input.get_files() {
        let name = file.to_str().unwrap()[root_offset..file.to_str().unwrap().len() - 3].to_string();
        let elements = match parser::parse(&name, fs::read_to_string(&file).unwrap()) {
            Ok(result) => result,
            Err(mut errors) => {
                failed.append(&mut errors);
                Vec::new()
            }
        };

        if name == "main" {
            match elements.iter().find(|element| {
                if let TopElement::Function(function) = element {
                    function.name == "main::main"
                } else {
                    false
                }
            }) {
                Some(_main) => output.main = Some("main::main".to_string()),
                None => {}
            }
        }
        for element in elements {
            println!("{}", element);
            match element {
                TopElement::Function(function) => {
                    output.static_functions.insert(function.name.clone(), function);
                }
                TopElement::Struct(structure) => {
                    output.elem_types.insert(structure.name.clone(), structure);
                }
            }
        }
    }

    if !failed.is_empty() {
        let mut errors = "Parsing Errors:\n".to_string();
        for error in failed {
            errors += format!("\n{}\n", error).as_str();
        }
        panic!("{}", errors);
    }
    return output;
}

pub trait FileStructure {
    fn get_files(&self) -> Vec<PathBuf>;

    fn get_root(&self) -> PathBuf;
}