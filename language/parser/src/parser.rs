extern crate pest;

use pest::iterators::Pairs;
use pest::Parser;
use ast::basic_types::Ident;
use ast::code::Effect;
use ast::r#struct::{Struct, TypeMember};
use ast::function::{CodeBody, Function};
use ast::TopElement;

#[derive(Parser)]
#[grammar = "language.pest"]
struct LanguageParser;

pub fn parse(input: String) -> Vec<TopElement> {
    let output = match LanguageParser::parse(Rule::element, input.as_str()) {
        Ok(result) => Vec::parse(result),
        Err(errors) => panic!("\n{}", errors)
    };

    return output;
}

pub trait Parsable {
    fn parse(rules: Pairs<Rule>) -> Self;
}

pub trait EffectParsable {
    fn parse(last: Option<Box<dyn Effect>>, rules: Pairs<Rule>) -> Self;
}

impl Parsable for Vec<TopElement> {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut output = Vec::new();
        for element in rules {
            match element.as_rule() {
                Rule::structure => output.push(TopElement::Struct(Struct::parse(element.into_inner()))),
                Rule::function => output.push(TopElement::Function(Function::parse(element.into_inner()))),
                Rule::EOI => {},
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }
        return output;
    }
}

impl Parsable for Struct {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut members: Vec<Box<dyn TypeMember>> = Vec::new();
        let mut name = String::new();

        for element in rules {
            match element.as_rule() {
                Rule::ident => name = element.as_str().to_string(),
                Rule::struct_field => {},
                Rule::function => members.push(Box::new(Function::parse(element.into_inner()))),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }
        return Struct::new(members, &[], Ident::new(name));
    }
}