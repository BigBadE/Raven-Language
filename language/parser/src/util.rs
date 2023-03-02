use ast::code::Field;
use ast::function::{Arguments, CodeBody};
use ast::program::Program;
use crate::code::{parse_effect, parse_expression};
use crate::parser::ParseInfo;
use crate::types::ParsingTypeResolver;

pub fn parse_fields(parsing: &mut ParseInfo) -> Option<Vec<Field>> {
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

fn parse_field(string: String, parser: &mut ParseInfo) -> Option<Field> {
    let parts: Vec<&str> = string.split(':').collect();
    if parts.len() != 2 {
        parser.create_error("Missing or unexpected colon in field.".to_string());
        return None;
    }

    return Some(Field::new(parts[0].to_string(), parts[1].to_string()));
}

pub fn parse_code_block(program: &Program, type_manager: &ParsingTypeResolver, parsing: &mut ParseInfo) -> Option<CodeBody> {
    if let None = parsing.parse_to(b'{') {
        parsing.create_error("Expected code body".to_string());
        return None;
    }

    let mut expressions = Vec::new();
    while let Some(expression) = parse_expression(program, type_manager, parsing) {
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

pub fn parse_arguments(program: &Program, type_manager: &ParsingTypeResolver, parsing: &mut ParseInfo) -> Arguments {
    let mut output = Vec::new();
    while let Some(effect) = parse_effect(program, type_manager, parsing, &[b',', b')']) {
        output.push(effect);
    }
    return Arguments::new(output);
}