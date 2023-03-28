use ast::blocks::{ForStatement, IfStatement, SwitchStatement};
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

    while parsing.matching("elseif") {
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

    match statement.condition.unwrap().return_type() {
        Some(return_type) => if return_type.to_string() != "bool" {
            parsing.create_error("If expression isn't a boolean".to_string());
            return None;
        },
        None => {
            parsing.create_error("If expression has void return type".to_string());
            return None;
        }
    }

    let expected = statement.body.return_type();
    match &statement.else_body {
        Some(effect) => {
            if effect.return_type() != expected {
                parsing.create_error("Else has different return type from If".to_string());
            }
        }
        None => {}
    }

    for (body, expression) in &statement.else_ifs {
        match expression.unwrap().return_type() {
            Some(return_type) => if return_type.to_string() != "bool" {
                parsing.create_error("Else If expression isn't a boolean".to_string());
                return None;
            },
            None => {
                parsing.create_error("Else If expression has void return type".to_string());
                return None;
            }
        }

        if body.return_type() != expected {
            parsing.create_error("Else If has different return type from If".to_string());
        }
    }

    return Some(Effects::IfStatement(Box::new(statement)));
}

pub fn parse_for(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Option<Effects> {
    parsing.next_included();
    parsing.index -= 1;
    let var_name = match parsing.parse_to_space() {
        Some(found) => found,
        None => {
            parsing.create_error("Expected variable name in for".to_string());
            return None;
        }
    };

    if !parsing.matching("in") {
        parsing.create_error("For loop needs \"in\"".to_string());
        return None;
    }

    let iterating = parse_effect(type_manager, parsing, &[b'{', b'}', b';']);

    if parsing.buffer[parsing.index-1] != b'{' {
        parsing.create_error("Unexpected end to for loop statement!".to_string());
        return None;
    }
    let iterating = match iterating {
        Some(iterating) => iterating,
        None => {
            parsing.create_error("Couldn't find effect!".to_string());
            return None;
        }
    };

    parsing.index -= 1;
    let code = match parse_code_block(type_manager, parsing) {
        Some(code) => code,
        None => {
            parsing.create_error("Expected code body".to_string());
            return None;
        }
    };
    return Some(Effects::ForStatement(Box::new(ForStatement::new(var_name, iterating, code))));
}

pub fn parse_switch(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Option<Effects> {
    let effect = match parse_effect(type_manager, parsing, &[b'{', b'}', b';']) {
        Some(effect) => effect,
        None => {
            parsing.create_error("Expected effect!".to_string());
            return None;
        }
    };

    if parsing.buffer[parsing.index-1] != b'{' {
        parsing.create_error("Unexpected end to switch!".to_string());
        return None;
    }

    let mut conditions = Vec::new();
    while !parsing.matching("}") {
        let condition = match parse_effect(type_manager, parsing, &[b'{', b'}', b';']) {
            Some(effect) => effect,
            None => {
                parsing.create_error("Expected effect!".to_string());
                return None;
            }
        };
        if !parsing.matching("=>") {
            parsing.create_error("Expected => before switch case body".to_string());
            return None;
        }
        let body;
        if parsing.matching("{") {
            parsing.index -= 1;
            body = match parse_code_block(type_manager, parsing) {
                Some(body) => Effects::CodeBody(Box::new(body)),
                None => {
                    parsing.create_error("Expected code body!".to_string());
                    return None;
                }
            };
        } else {
            body = match parse_effect(type_manager, parsing, &[b',', b'}']) {
                Some(body) => body,
                None => {
                    parsing.create_error("Expected effect!".to_string());
                    return None;
                }
            };
        }
        conditions.push((condition, body));
    }

    return Some(Effects::SwitchStatement(Box::new(SwitchStatement::new(effect, conditions, parsing.loc()))));
}