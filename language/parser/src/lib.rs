#[macro_use]
extern crate pest_derive;

use ast::TopElement;

pub mod parser;

pub fn parse(input: String) -> Vec<TopElement> {
    let elements = parser::parse(input);
    for element in &elements {
        println!("{}", element);
    }
    return elements;
}