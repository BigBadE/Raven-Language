use std::str::FromStr;
use std::fmt::{Debug, Display};
use pest::iterators::Pairs;
use ast::code::{AssignVariable, Effects, Expression, ExpressionType, Field, MathEffect, MathOperator, NumberEffect, VariableLoad};
use ast::function::{CodeBody, Function};
use ast::Modifier;
use crate::parser::{EffectParsable, EXPRESSION_PARSER, Parsable, Rule};

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
                        return_type = Some(element.as_str().to_string());
                    }
                }
                Rule::fields => fields = Vec::parse(element.into_inner()),
                Rule::code_block => code = CodeBody::parse(element.into_inner()),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        return Function::new(modifiers.as_slice(), fields, code, return_type, name);
    }
}

impl Parsable for CodeBody {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut expressions = Vec::new();
        'outer: for element in rules {
            match element.as_rule() {
                Rule::expression => expressions.push(Expression::parse(element.into_inner())),
                Rule::escape_statement => {
                    let mut expression_type = ExpressionType::Line;
                    for element in element.into_inner() {
                        match element.as_rule() {
                            Rule::returning => expression_type = ExpressionType::Return,
                            Rule::block_return => expression_type = ExpressionType::Break,
                            Rule::wrapped_effect => {
                                expressions.push(
                                    Expression::new(expression_type, Expression::parse(element.into_inner()).effect));
                                continue 'outer;
                            },
                            _ => panic!("Unimplemented rule!: {}", element)
                        }
                    }
                    expressions.push(Expression::new(expression_type, Effects::NOP()));
                },
                Rule::block => {

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
        EXPRESSION_PARSER.map_primary(|primary| match primary.as_rule() {
            _ => panic!("Unimplemented rule!: {}", primary)
        }).map_infix(|lhs, op, rhs| {
            match op.as_rule() {
                _ => panic!("Unimplemented rule!: {}", op)
            }
        }).parse(rules);

        /*
        for element in rules {
            match element.as_rule() {
                Rule::effect => last = Some(Effects::parse(last, element.into_inner())),
                Rule::ident => last = Some(Effects::VariableLoad(Box::new(VariableLoad::new(element.as_str().to_string())))),
                Rule::math => last = Some(Effects::MathEffect(Box::new(MathEffect::parse(last, element.into_inner())))),
                Rule::float => last = Some(Effects::FloatEffect(Box::new(parse_number::<f64>(element.as_str())))),
                Rule::integer => last = Some(Effects::IntegerEffect(Box::new(parse_number::<i64>(element.as_str())))),
                Rule::assign => last = Some(Effects::AssignVariable(Box::new(AssignVariable::parse(element.into_inner())))),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }*/

        return Expression::new(ExpressionType::Line, last.unwrap());
    }
}

//The error must implement Debug, which is why this looks so god awful
fn parse_number<T>(number: &str) -> NumberEffect<T> where T: Display + FromStr, <T as FromStr>::Err: Display + Debug {
    return match number.parse::<T>() {
        Ok(numb) => NumberEffect::new(numb),
        Err(error) => panic!("Error parsing number \"{}\": {}", number, error)
    };
}

impl Parsable for AssignVariable {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut variable = String::new();
        let mut effects = None;
        let mut given_type = None;
        for element in rules {
            match element.as_rule() {
                Rule::ident => variable = element.as_str().to_string(),
                Rule::assign_type => given_type = Some(element.into_inner().last().unwrap().as_str().to_string()),
                Rule::wrapped_effect => effects = Some(Expression::parse(element.into_inner()).effect),
                _ => panic!("Unimplemented rule! {}", element)
            }
        }
        return AssignVariable::new(variable, given_type, effects.unwrap())
    }
}

impl EffectParsable for MathEffect {
    fn parse(last: Option<Effects>, rules: Pairs<Rule>) -> Self {
        let mut operation = MathOperator::PLUS;
        for element in rules {
            match element.as_rule() {
                Rule::addition => {}
                Rule::subtraction => operation = MathOperator::MINUS,
                Rule::division => operation = MathOperator::DIVIDE,
                Rule::multiplication => operation = MathOperator::MULTIPLY,
                Rule::wrapped_effect => return MathEffect::new(last, operation,
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
                        return Field::new(name, element.as_str().to_string());
                    }
                }
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }

        panic!("Invalid field!");
    }
}