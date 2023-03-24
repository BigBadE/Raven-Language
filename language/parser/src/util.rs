use std::collections::HashMap;

use ast::code::{Effects, Field};
use ast::function::{Arguments, CodeBody};
use ast::type_resolver::TypeResolver;
use ast::types::ResolvableTypes;

use crate::code::{parse_effect, parse_expression};
use crate::parser::ParseInfo;

pub fn parse_fields(parent: Option<String>, parsing: &mut ParseInfo) -> Option<Vec<Field>> {
    let mut output = Vec::new();
    let mut info = parsing.clone();
    while let Some(found) = find_if_first(parsing, b',', b')') {
        output.push(match parse_field(&parent, found, &mut info) {
            Some(found) => found,
            None => {
                parsing.create_error(info.errors.get(0).unwrap().error.clone());
                return None;
            }
        });
        info = parsing.clone();
    }

    if let Some(found) = parsing.parse_to(b')') {
        if !found.is_empty() {
            output.push(match parse_field(&parent, found, &mut info) {
                Some(found) => found,
                None => {
                    parsing.create_error(info.errors.get(0).unwrap().error.clone());
                    return None;
                }
            })
        }
    }

    return Some(output);
}

pub fn parse_struct_fields() {}

fn parse_field(parent: &Option<String>, string: String, parser: &mut ParseInfo) -> Option<Field> {
    let mut parts = string.split(':');
    let name = parts.next().unwrap();
    if name.len() == string.len() {
        parser.create_error("Missing type for field.".to_string());
        return None;
    }

    let body = &string[name.len()+1..].to_string().replace(" ", "");
    if body.contains(")") {
        if name == "self" {
            match parent {
                Some(parent) => return Some(Field::new("self".to_string(), ResolvableTypes::Resolving(parent.clone()))),
                None => parser.create_error("Cannot have self outside of struct!".to_string())
            }
        } else {
            parser.create_error("Field missing type!".to_string());
        }
        return None;
    }
    return Some(Field::new(name.to_string(), ResolvableTypes::Resolving(body.to_string())));
}

pub fn parse_code_block(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Option<CodeBody> {
    if let None = parsing.parse_to(b'{') {
        parsing.create_error("Expected code body".to_string());
        return None;
    }

    let mut expressions = Vec::new();
    while let Some(expression) = parse_expression(type_manager, parsing) {
        expressions.push(expression);

        match parsing.next_included() {
            Some(found) => if found == b'}' {
                break;
            } else {
                parsing.index -= 1;
            }
            None => break
        }
    }

    return Some(CodeBody::new(expressions));
}

pub fn get_line(buffer: &[u8], start: usize) -> String {
    for i in start..buffer.len() {
        if buffer[i] == b'\n' {
            return String::from_utf8_lossy(&buffer[start..i]).to_string();
        }
    }
    return String::from_utf8_lossy(&buffer[start..]).to_string();
}

pub fn find_if_first(parsing: &mut ParseInfo, first: u8, second: u8) -> Option<String> {
    let mut parse_clone = parsing.clone();
    if let Some(_) = parse_clone.parse_to(second) {
        if let Some(_) = parsing.clone().parse_to_or_end(first, parse_clone.index) {
            return Some(parsing.parse_to(first).unwrap());
        }
    }
    return None;
}

pub fn parse_arguments(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Arguments {
    let mut output = Vec::new();
    if parsing.buffer[parsing.index] == b')' {
        return Arguments::new(output);
    }
    while parsing.buffer[parsing.index - 1] != b')' {
        if let Some(effect) = parse_effect(type_manager, parsing, &[b',', b')']) {
            output.push(effect);
        } else {
            parsing.create_error("Missing effect!".to_string());
            break;
        }
    }
    return Arguments::new(output);
}

pub fn parse_struct_args(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Vec<(String, Effects)> {
    let mut output = Vec::new();
    while parsing.len != parsing.index && !parsing.matching("}") {
        let found_name;
        if let Some(name) = find_if_first(parsing, b':', b',') {
            found_name = name;
        } else if let Some(name) = find_if_first(parsing, b':', b'}') {
            found_name = name;
        } else {
            if let Some(_name) = find_if_first(&mut parsing.clone(), b',', b'}') {
                found_name = parsing.parse_to(b',').unwrap();
            } else {
                found_name = match parsing.parse_to(b'}') {
                    Some(name) => name,
                    None => {
                        parsing.create_error("Missing end to Struct parameters!".to_string());
                        return output;
                    }
                };
                parsing.index -= 1;
            }
        }

        output.push((found_name, parse_effect(type_manager, parsing, &[b',', b';', b'}']).expect("No effect!")));
        parsing.index -= 1;
        if (parsing.buffer[parsing.index - 1] == b'}' && !parsing.find_next(b'}')) || parsing.buffer[parsing.index - 1] == b';' {
            parsing.create_error("Missing comma after structure initializer value".to_string());
            return output;
        }
        parsing.matching(",");
    }
    return output;
}

pub fn parse_generics(parsing: &mut ParseInfo, generics: &mut HashMap<String, Vec<ResolvableTypes>>) {
    while let Some(value) = find_if_first(parsing, b',', b'>') {
        let (name, val) = parse_generic(value);
        generics.insert(name, val);
    }

    if let Some(value) = parsing.parse_to(b'>') {
        let (name, val) = parse_generic(value);
        generics.insert(name, val);
    } else {
        panic!("Expected generic!");
    }
}

pub fn parse_generics_vec(parsing: &mut ParseInfo, generics: &mut Vec<(String, Vec<ResolvableTypes>)>) {
    while let Some(value) = find_if_first(parsing, b',', b'>') {
        generics.push(parse_generic(value));
    }

    if let Some(value) = parsing.parse_to(b'>') {
        generics.push(parse_generic(value));
    } else {
        panic!("Expected generic!");
    }
}

pub fn parse_generic(value: String) -> (String, Vec<ResolvableTypes>) {
    let mut split = value.split(':');
    let name = split.next().unwrap();
    let mut found = Vec::new();
    match split.next() {
        Some(constraint) => {
            let mut constraints = constraint.split('+');
            while let Some(constraint) = constraints.next() {
                found.push(ResolvableTypes::Resolving(constraint.to_string().replace(" ", "")));
            }
        }
        None => {}
    }
    return (name.to_string(), found);
}