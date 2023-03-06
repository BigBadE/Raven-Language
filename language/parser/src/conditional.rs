use ast::blocks::IfStatement;
use ast::code::{Effect, Effects};
use ast::type_resolver::TypeResolver;
use crate::code::parse_effect;
use crate::parser::ParseInfo;
use crate::util::parse_code_block;

pub fn parse_if(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Option<Effects> {
    let type_manager = type_manager.clone();
    let effect = parse_effect(type_manager, parsing, &[b'{', b'}', b';']);
    parsing.index -= 1;

    let mut statement;
    match effect {
        Some(effect) => match parse_code_block(type_manager, parsing) {
            Some(body) => statement = IfStatement::new(body, effect, parsing.loc()),
            None => {
                parsing.create_error("If statement lacks body".to_string());
                return None;
            }
        },
        None => {
            parsing.create_error("If statement lacks a condition".to_string());
            return None;
        }
    }

    while parsing.matching("else if") {
        let effect = parse_effect(type_manager, parsing, &[b'{', b'}', b';']);
        parsing.index -= 1;
        match effect {
            Some(effect) => match parse_code_block(type_manager, parsing) {
                Some(body) => statement.else_ifs.push((body, effect)),
                None => {
                    parsing.create_error("Else If statement lacks body".to_string());
                    return None;
                }
            },
            None => {
                parsing.create_error("Else If statement lacks a condition".to_string());
                return None;
            }
        }
    }

    if parsing.matching("else") {
        match parse_code_block(type_manager, parsing) {
            Some(body) => statement.else_body = Some(body),
            None => {
                parsing.create_error("Else statement lacks body".to_string());
                return None;
            }
        }
    }

    match statement.condition.unwrap().return_type(type_manager) {
        Some(return_type) => if return_type.name != "bool" {
            parsing.create_error("If expression isn't a boolean".to_string());
            return None;
        },
        None => {
            parsing.create_error("If expression has void return type".to_string());
            return None;
        }
    }

    let expected = statement.body.return_type(type_manager);
    match &statement.else_body {
        Some(effect) => {
            if effect.return_type(type_manager) != expected {
                parsing.create_error("Else has different return type from If".to_string());
            }
        }
        None => {}
    }

    for (body, expression) in &statement.else_ifs {
        match expression.unwrap().return_type(type_manager) {
            Some(return_type) => if return_type.name != "bool" {
                parsing.create_error("Else If expression isn't a boolean".to_string());
                return None;
            },
            None => {
                parsing.create_error("Else If expression has void return type".to_string());
                return None;
            }
        }

        if body.return_type(type_manager) != expected {
            parsing.create_error("Else If has different return type from If".to_string());
        }
    }

    return Some(Effects::IfStatement(Box::new(statement)));
}