use std::collections::HashMap;
use ast::code::{Effects, Field};
use ast::function::{Arguments, CodeBody};
use ast::type_resolver::TypeResolver;
use ast::types::ResolvableTypes;
use crate::code::{parse_effect, parse_expression};
use crate::parser::ParseInfo;

pub fn parse_fields<'a>(parsing: &mut ParseInfo) -> Option<Vec<Field>> {
    let mut output = Vec::new();
    let mut info = parsing.clone();
    while let Some(found) = find_if_first(parsing, b',', b')') {
        output.push(parse_field(found, &mut info)?);
        info = parsing.clone();
    }

    if let Some(found) = parsing.parse_to(b')') {
        if !found.is_empty() {
            output.push(parse_field(found, &mut info)?)
        }
    }

    return Some(output);
}

pub fn parse_struct_fields() {}

fn parse_field<'a>(string: String, parser: &mut ParseInfo) -> Option<Field> {
    let parts: Vec<&str> = string.split(':').collect();
    if parts.len() != 2 {
        parser.create_error("Missing or unexpected colon in field.".to_string());
        return None;
    }

    return Some(Field::new(parts[0].to_string(), ResolvableTypes::Resolving(parts[1].to_string())));
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
    while parsing.buffer[parsing.index-1] != b')' {
        if let Some(effect) = parse_effect(type_manager, parsing, &[b',', b')']) {
            output.push(effect);
        } else {
            parsing.create_error("Missing effect!".to_string());
            break
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

pub fn parse_generics(parsing: &mut ParseInfo, generics: &mut HashMap<String, Vec<String>>) {
    while let Some(value) = find_if_first(parsing, b',', b'>') {
        parse_generic(value, generics);
    }

    if let Some(value) = parsing.parse_to(b'>') {
        parse_generic(value, generics);
    } else {
        panic!("Expected generic!");
    }
}

pub fn parse_generic(value: String, generics: &mut HashMap<String, Vec<String>>) {
    let mut split = value.split(':');
    let name = split.next().unwrap();
    let mut found = Vec::new();
    match split.next() {
        Some(constraint) => {
            let mut constraints = constraint.split('+');
            while let Some(constraint) = constraints.next() {
                found.push(constraint.to_string().replace(" ", ""));
            }
        },
        None => {}
    }
    generics.insert(name.to_string(), found);
}