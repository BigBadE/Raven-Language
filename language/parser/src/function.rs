use pest::iterators::Pairs;
use ast::basic_types::Ident;
use ast::code::{Expression, Field};
use ast::function::{Arguments, CodeBody, Function};
use ast::Modifier;
use crate::parser::{EffectParsable, Parsable, Rule};

impl Parsable for Function {
    fn parse(function: Pairs<Rule>) -> Function {
        let mut modifiers = Vec::new();
        let mut code = CodeBody::default();
        let mut name = String::new();
        let mut fields = Vec::new();

        for element in function {
            match element.as_rule() {
                Rule::modifiers => modifiers = Vec::parse(element.into_inner()),
                Rule::ident => name = element.as_str().to_string(),
                Rule::fields => fields = Vec::parse(element.into_inner()),
                Rule::code_block => code = CodeBody::parse(element.into_inner()),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return Function::new(modifiers.as_slice(), fields, code, Ident::new(name));
    }
}

impl Parsable for CodeBody {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut expressions = Vec::new();
        for element in rules {
            match element.as_rule() {
                Rule::expression => expressions.push(Expression::parse(element.into_inner())),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return CodeBody::new(expressions);
    }
}

impl Parsable for Expression {
    fn parse(rules: Pairs<Rule>) -> Expression {
        let mut last = None;
        for element in rules {
            match element.as_rule() {
                Rule::effect => last = Some(Box::parse(last, element.into_inner())),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return Expression::new(last.unwrap());
    }
}

impl Parsable for Vec<Modifier> {
    fn parse(rules: Pairs<Rule>) -> Vec<Modifier> {
        let mut output = Vec::new();
        for element in rules {
            match element.as_rule() {
                Rule::public => output.push(Modifier::Public),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }
        return output;
    }
}

impl Parsable for Vec<Field> {
    fn parse(rules: Pairs<Rule>) -> Vec<Field> {
        let mut output = Vec::new();
        for element in rules {
            match element.as_rule() {
                Rule::field => output.push(Field::parse(element.into_inner())),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }
        return output;
    }
}

impl Parsable for Field {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut name = String::new();
        for element in rules {
            match element.as_rule() {
                Rule::ident => {
                    if name.is_empty() {
                        name = element.as_str().to_string();
                    } else {
                        return Field::new(Ident::new(name), Ident::new(element.as_str().to_string()));
                    }
                }
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        panic!("Invalid field!");
    }
}