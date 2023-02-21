#[macro_use]
extern crate pest_derive;
extern crate core;

use ast::TopElement;

pub mod code;
pub mod function;
pub mod parser;

pub fn parse(input: String) -> Vec<TopElement> {
    let elements = parser::parse(input);
    for element in &elements {
        println!("{}", element);
    }
    return elements;
}