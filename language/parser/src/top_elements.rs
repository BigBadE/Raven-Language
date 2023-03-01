use std::ops::Deref;
use ast::function::Function;
use ast::r#struct::Struct;
use ast::{Modifier, TopElement};
use crate::parser::{ParseError, ParseInfo};
use crate::util::{parse_code_block, parse_fields};

pub fn parse_top_element(name: &String, errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<TopElement> {
    let modifiers = parse_modifiers(parsing)?;
    if parsing.matching("struct") {
        return match parse_struct(name, modifiers.deref(), errors, parsing) {
            Some(structure) => Some(TopElement::Struct(structure)),
            None => None
        };
    } else if parsing.matching("fn") {
        return match parse_function(name, modifiers.deref(), errors, parsing) {
            Some(function) => Some(TopElement::Function(function)),
            None => None
        };
    }
    errors.push(parsing.create_error("Unknown element".to_string()));
    return None;
}

fn parse_struct(name: &String, modifiers: &[Modifier], errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<Struct> {
    todo!()
}

fn parse_function(name: &String, modifiers: &[Modifier], errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<Function> {
    let name = name.clone() + "::" + match parsing.parse_to(b'(') {
        Some(name) => name.as_str(),
        None => {
            errors.push(parsing.create_error("Expected string name".to_string()));
            return None;
        }
    };

    let fields = match parse_fields(errors, parsing) {
        Some(fields) => fields,
        None => return None
    };

    let return_type = if let Some(value) = parsing.matching("->") {
        if value {
            match parsing.parse_to(b'{') {
                Some(found) => {
                    parsing.index -= 1;
                    Some(found)
                },
                None => {
                    errors.push(parsing.create_error("Expected code body".to_string()));
                    return None;
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let code = match parse_code_block(errors, parsing) {
        Some(code) => code,
        None => return None
    };

    return Some(Function::new(modifiers, fields, code, return_type, name));
}

fn parse_modifiers(parsing: &mut ParseInfo) -> Option<Vec<Modifier>> {
    let mut modifiers = Vec::new();
    if parsing.matching("pub")? {
        modifiers.push(Modifier::Public);
    }
    return Some(modifiers);
}