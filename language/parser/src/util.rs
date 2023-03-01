use ast::code::Field;
use ast::function::CodeBody;
use crate::code::parse_expression;
use crate::parser::{ParseError, ParseInfo};

pub fn parse_fields(errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<Vec<Field>> {
    let mut output = Vec::new();
    let mut info = parsing.clone();
    while let Some(found) = parsing.parse_to(b',') {
        output.push(parse_field(errors, found, &mut info)?);
        info = parsing.clone();
    }

    info = parsing.clone();
    if let Some(found) = parsing.parse_to(b')') {
        if !found.is_empty() {
            output.push(parse_field(errors, found, &mut info)?)
        }
        return Some(output);
    }

    errors.push(parsing.create_error("".to_string()));
    return None;
}

fn parse_field(errors: &mut Vec<ParseError>, string: String, parser: &mut ParseInfo) -> Option<Field> {
    let parts: Vec<&str> = string.split(':').collect();
    if parts.len() != 2 {
        errors.push(parser.create_error("Missing or unexpected colon in field.".to_string()));
        return None;
    }

    return Some(Field::new(parts[0].to_string(), parts[1].to_string()));
}

pub fn parse_code_block(errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<CodeBody> {
    if let None = parsing.parse_to(b'{') {
        errors.push(parsing.create_error("Expected code body".to_string()));
        return None;
    }

    let mut expressions = Vec::new();
    while let Some(expression) = parse_expression(errors, parsing) {
        expressions.push(expression);
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