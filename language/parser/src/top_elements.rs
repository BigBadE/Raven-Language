use std::ops::Deref;
use ast::function::{CodeBody, Function};
use ast::r#struct::Struct;
use ast::Modifier;
use ast::program::Program;
use crate::parser::ParseInfo;
use crate::types::ParsingTypeResolver;
use crate::util::{parse_code_block, parse_fields};

pub fn parse_top_elements(program: &mut Program, name: &String, parsing: &mut ParseInfo, parse_code: bool) {
    while let Some(_) = parsing.next_included() {
        parsing.index -= 1;
        let modifiers = parse_modifiers(parsing);
        if parsing.matching("struct") {
            match parse_struct(program, name, modifiers.deref(), parsing, parse_code) {
                Some(structure) => program.elem_types.insert(name.clone(), structure),
                None => None
            };
            continue
        } else if parsing.matching("fn") {
            match parse_function(program, name, modifiers.deref(), parsing, parse_code) {
                Some(function) => program.static_functions.insert(name.clone(), function),
                None => None
            };
            continue
        }

        //Only error once for a big block of issues.
        if parsing.errors.last().is_none() || parsing.errors.last().unwrap().error != "Unknown element" {
            parsing.create_error("Unknown element".to_string());
        } else {
            parsing.skip_line();
        }
    }
}

fn parse_struct(program: &Program, name: &String, modifiers: &[Modifier], parsing: &mut ParseInfo, parse_code: bool) -> Option<Struct> {
    todo!()
}

fn parse_function(program: &Program, name: &String, modifiers: &[Modifier], parsing: &mut ParseInfo, parse_code: bool) -> Option<Function> {
    let type_manager = ParsingTypeResolver::new(program);
    let name = name.clone() + "::" + match parsing.parse_to(b'(') {
        Some(name) => name.clone(),
        None => {
            parsing.create_error("Expected string name".to_string());
            return None;
        }
    }.as_str();

    let fields = match parse_fields(parsing) {
        Some(fields) => fields,
        None => return None
    };

    let return_type = if parsing.matching("->") {
        match parsing.parse_to(b'{') {
            Some(found) => {
                parsing.index -= 1;
                Some(found)
            }
            None => {
                parsing.create_error("Expected code body".to_string());
                return None;
            }
        }
    } else {
        None
    };

    let code = if parse_code {
        match parse_code_block(program, &type_manager, parsing) {
            Some(code) => code,
            None => return None
        }
    } else {
        CodeBody::new(Vec::new())
    };

    return Some(Function::new(modifiers, fields, code, return_type, name));
}

fn parse_modifiers(parsing: &mut ParseInfo) -> Vec<Modifier> {
    let mut modifiers = Vec::new();
    while let Some(modifier) = parse_modifier(parsing) {
        modifiers.push(modifier);
    }
    return modifiers;
}

fn parse_modifier(parsing: &mut ParseInfo) -> Option<Modifier> {
    if parsing.matching("pub") {
        return Some(Modifier::Public);
    } else if parsing.matching("pub(proj)") {
        return Some(Modifier::Protected);
    } else if parsing.matching("extern") {
        return Some(Modifier::Extern);
    } else if parsing.matching("internal") {
        return Some(Modifier::Internal);
    } else if parsing.matching("operation") {
        return Some(Modifier::Operation);
    }
    return None;
}