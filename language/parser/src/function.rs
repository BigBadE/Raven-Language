use std::str::FromStr;
use std::fmt::{Debug, Display};
use pest::iterators::Pairs;
use ast::basic_types::Ident;
use ast::code::{Effect, Expression, Field, MathEffect, MathOperator, NumberEffect, ReturnEffect, VariableLoad};
use ast::function::{CodeBody, Function};
use ast::Modifier;
use crate::parser::{EffectParsable, Parsable, Rule};

impl Parsable for Function {
    fn parse(function: Pairs<Rule>) -> Function {
        let mut modifiers = Vec::new();
        let mut code = CodeBody::default();
        let mut name = String::new();
        let mut fields = Vec::new();
        let mut return_type = None;

        for element in function {
            match element.as_rule() {
                Rule::modifiers => modifiers = Vec::parse(element.into_inner()),
                Rule::ident => {
                    if name.is_empty() {
                        name = element.as_str().to_string();
                    } else {
                        return_type = Some(Ident::new(element.as_str().to_string()));
                    }
                }
                Rule::fields => fields = Vec::parse(element.into_inner()),
                Rule::code_block => code = CodeBody::parse(element.into_inner()),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return Function::new(modifiers.as_slice(), fields, code, return_type, Ident::new(name));
    }
}

impl Parsable for CodeBody {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut expressions = Vec::new();
        let mut returning = false;
        for element in rules {
            match element.as_rule() {
                Rule::returning => returning = true,
                Rule::expression => {
                    let mut expression = Expression::parse(element.into_inner());
                    if returning {
                        expression = Expression::new(Box::new(ReturnEffect::new(expression.effect)));
                        returning = false;
                    }
                    expressions.push(expression);
                }
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
                Rule::ident => {
                    println!("Loading a{}a", element.as_str());
                    last = Some(Box::new(VariableLoad::new(Ident::new(element.as_str().to_string()))))
                }
                Rule::math => last = Some(Box::new(MathEffect::parse(last, element.into_inner()))),
                Rule::float => last = Some(Box::new(parse_number::<f64>(element.as_str()))),
                Rule::integer => last = Some(Box::new(parse_number::<u64>(element.as_str()))),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return Expression::new(last.unwrap());
    }
}

//The error must implement Debug, which is why this looks so god awful
fn parse_number<T>(number: &str) -> NumberEffect<T> where T: Display + FromStr, <T as FromStr>::Err: Display + Debug {
    return match number.parse::<T>() {
        Ok(numb) => NumberEffect::new(numb),
        Err(error) => panic!("Error parsing number \"{}\": {}", number, error)
    };
}

impl EffectParsable for MathEffect {
    fn parse(last: Option<Box<dyn Effect>>, rules: Pairs<Rule>) -> Self {
        let mut operation = MathOperator::PLUS;
        for element in rules {
            match element.as_rule() {
                Rule::addition => {}
                Rule::subtraction => operation = MathOperator::MINUS,
                Rule::division => operation = MathOperator::DIVIDE,
                Rule::multiplication => operation = MathOperator::MULTIPLY,
                Rule::wrapped_effect => return MathEffect::new(last.unwrap(), operation,
                                                               Expression::parse(element.into_inner()).effect),
                _ => panic!("Unimplemented rule! {}", element)
            }
        }

        panic!("Failed to find effect!");
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