extern crate pest;

use lazy_static::lazy_static;
use pest::iterators::Pairs;
use pest::Parser;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use ast::code::Effects;
use ast::r#struct::{Struct, TypeMembers};
use ast::function::Function;
use ast::TopElement;

#[derive(Parser)]
#[grammar = "language.pest"]
struct LanguageParser;

lazy_static! {
    pub static ref EXPRESSION_PARSER: PrattParser<Rule> = PrattParser::new()
            .op(Op::infix(Rule::terms, Assoc::Left))
            .op(Op::infix(Rule::assign, Assoc::Left));

    pub static ref MATH_PARSER: PrattParser<Rule> = PrattParser::new()
            .op(Op::infix(Rule::multiplication, Assoc::Left) | Op::infix(Rule::division, Assoc::Left))
            .op(Op::infix(Rule::addition, Assoc::Left) | Op::infix(Rule::subtraction, Assoc::Left));
}

pub fn parse(name: &String, input: String) -> Vec<TopElement> {
    let output = match LanguageParser::parse(Rule::element, input.as_str()) {
        Ok(result) => parse_root(name, result),
        Err(errors) => panic!("\n{}", errors)
    };

    return output;
}

pub trait Parsable {
    fn parse(rules: Pairs<Rule>) -> Self;
}

pub trait EffectParsable {
    fn parse(last: Option<Effects>, rules: Pairs<Rule>) -> Self;
}

fn parse_root(name: &String, rules: Pairs<Rule>) -> Vec<TopElement> {
    let mut output = Vec::new();
    for element in rules {
        match element.as_rule() {
            Rule::structure => output.push(TopElement::Struct(Struct::parse(element.into_inner()))),
            Rule::function => output.push(TopElement::Function(Function::parse(element.into_inner()))),
            Rule::EOI => {}
            _ => panic!("Unimplemented rule!: {}", element)
        }
    }

    for element in &mut output {
        match element {
            TopElement::Struct(structure) => structure.name = name.clone() + "::" + structure.name.as_str(),
            TopElement::Function(function) => function.name = name.clone() + "::" + function.name.as_str()
        }
    }

    return output;
}

impl Parsable for Struct {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut members: Vec<TypeMembers> = Vec::new();
        let mut name = String::new();

        for element in rules {
            match element.as_rule() {
                Rule::ident => name = element.as_str().to_string(),
                Rule::struct_field => {}
                Rule::function => members.push(TypeMembers::Function(Function::parse(element.into_inner()))),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return Struct::new(members, &[], name);
    }
}