extern crate pest;

use pest::iterators::Pair;
use pest::Parser;
use ast::basic_types::Ident;
use ast::class_type::{ClassType, TypeMember};
use ast::function_type::{CodeBody, Function};
use ast::TopElement;

#[derive(Parser)]
#[grammar = "language.pest"]
struct LanguageParser;

pub fn parse(input: String) -> Vec<TopElement> {
    let mut output: Vec<TopElement> = Vec::new();

    match LanguageParser::parse(Rule::element, input.as_str()) {
        Ok(result) => {
            for element in result {
                match element.as_rule() {
                    Rule::structure => output.push(TopElement::Struct(parse_structure(element))),
                    Rule::function => output.push(TopElement::Function(parse_function(element))),
                    _ => panic!("Unimplemented rule!: {}", element)
                }
            }
        },
        Err(errors) => {
            panic!("\n{}", errors);
        }
    }

    return output;
}

fn parse_structure(structure: Pair<Rule>) -> ClassType {
    let mut members: Vec<Box<dyn TypeMember>> = Vec::new();
    let mut name = String::new();

    for element in structure.into_inner() {
        match element.as_rule() {
            Rule::ident => name = element.as_str().to_string(),
            Rule::struct_field => {},
            Rule::function => members.push(Box::new(parse_function(element))),
            _ => panic!("Unimplemented rule!: {}", element)
        }
    }
    return ClassType::new(members, &[],Ident::new(name));
}

fn parse_function(function: Pair<Rule>) -> Function {
    let code = Vec::new();
    let name = String::new();

    match function.clone().into_inner() {
        _ => panic!("Unimplemented rule!: {}", function)
    }

    return Function::new(&[], CodeBody::new(code), Ident::new(name));
}