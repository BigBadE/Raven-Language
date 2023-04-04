use std::collections::HashMap;
use std::future::Future;
use std::hash;
use std::sync::{Arc, Mutex};

use syntax::code::{Effects, Expression, Field};
use syntax::function::{Arguments, CodeBody};
use syntax::ParsingError;
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use syntax::type_resolver::TypeResolver;
use syntax::types::{ResolvableTypes, Types};

use crate::code::{parse_effect, parse_expression};
use crate::imports::ImportManager;
use crate::parser::ParseInfo;

pub fn parse_fields(parent: Option<String>, parsing: &mut ParseInfo) -> Result<Vec<(String, String)>, ParsingError> {
    let mut output = Vec::new();
    let mut info = parsing.clone();
    while let Some(found) = find_if_first(parsing, b',', b')') {
        output.push(parse_field(&parent, found)?);
        info = parsing.clone();
    }

    if let Some(found) = parsing.parse_to(b')') {
        if !found.is_empty() {
            output.push(parse_field(&parent, found)?)
        }
    }

    return Ok(output);
}

pub fn parse_struct_fields() {}

fn parse_field(parent: &Option<String>, string: String) -> Result<(String, String), ParsingError> {
    let mut parts = string.split(':');
    let name = parts.next().unwrap();
    if name.len() == string.len() {
        return Err(ParsingError::new((0, 0), (0, 0), "Missing type for field.".to_string()));
    }

    let body = &string[name.len() + 1..].to_string().replace(" ", "");
    if body.contains(")") {
        return if name == "self" {
            match parent {
                Some(parent) => Ok(("self".to_string(), parent.clone())),
                None =>
                    Err(ParsingError::new((0, 0), (0, 0), "Cannot have self outside of struct!".to_string()))
            }
        } else {
            Err(ParsingError::new((0, 0), (0, 0), "Missing type for field.".to_string()))
        };
    }
    return Ok((name.to_string(), body.to_string()));
}

/// Async code parsing. Parsing must continue while other parts wait to prevent deadlocks.
/// To do this, the method provides a "done" callback it will use for the finalized result, but will
/// return as soon as parsing is finished.
pub fn parse_code_block(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                        -> Result<impl Future<Output=CodeBody>, ParsingError> {
    if let None = parsing.parse_to(b'{') {
        return Err(ParsingError::new((0, 0), (0, 0), "Expected code body".to_string()));
    }

    let mut expressions = Vec::new();
    while let Some(expression) = parse_expression(syntax, import_manager, parsing) {
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

    import_manager.code_block_id += 1;
    let handle = syntax.lock().unwrap().manager.handle().clone();
    return Ok(handle.spawn(async_code_block(expressions, import_manager.code_block_id.to_string())));
}

async fn async_code_block(expressions: Vec<impl Future<Output=Expression>>, label: String) -> CodeBody {
    let mut parsed = Vec::new();
    for expression in expressions {
        parsed.push(expression.await);
    }
    return CodeBody::new(parsed, label);
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

pub fn parse_arguments(syntax: &Arc<Mutex<Syntax>>, import_manger: &mut ImportManager, parsing: &mut ParseInfo)
                       -> Result<Vec<impl Future<Output=Effects>>, ParsingError> {
    let mut output = Vec::new();
    if parsing.matching(")") {
        return Ok(output);
    }
    while parsing.buffer[parsing.index - 1] != b')' {
        output.push(parse_effect(syntax, import_manger, parsing, &[b',', b')'])?);
    }
    return Ok(output);
}

pub fn parse_struct_args(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
    -> Result<Vec<(String, impl Future<Output=Effects>)>, ParsingError> {
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
                        return Err(ParsingError::new((0, 0), (0, 0), "Missing end to Struct parameters!".to_string()));
                    }
                };
                parsing.index -= 1;
            }
        }

        output.push((found_name, parse_effect(syntax, import_manager, parsing, &[b',', b';', b'}'])?));
        parsing.index -= 1;
        if (parsing.buffer[parsing.index - 1] == b'}' && !parsing.find_next(b'}')) || parsing.buffer[parsing.index - 1] == b';' {
            return Err(ParsingError::new((0, 0), (0, 0), "Missing comma after structure initializer value".to_string()));
        }
        parsing.matching(",");
    }
    return Ok(output);
}

pub fn parse_generics(parsing: &mut ParseInfo, generics: &mut HashMap<String, Vec<String>>) {
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

pub fn parse_generics_vec(parsing: &mut ParseInfo, generics: &mut Vec<(String, Vec<String>)>) {
    while let Some(value) = find_if_first(parsing, b',', b'>') {
        generics.push(parse_generic(value));
    }

    if let Some(value) = parsing.parse_to(b'>') {
        generics.push(parse_generic(value));
    } else {
        panic!("Expected generic!");
    }
}

pub fn parse_generic(value: String) -> (String, Vec<String>) {
    let mut split = value.split(':');
    let name = split.next().unwrap();
    let mut found = Vec::new();
    match split.next() {
        Some(constraint) => {
            let mut constraints = constraint.split('+');
            while let Some(constraint) = constraints.next() {
                found.push(constraint.to_string().replace(" ", ""));
            }
        }
        None => {}
    }
    return (name.to_string(), found);
}