use std::collections::HashMap;
use ast::function::{CodeBody, Function};
use ast::{Attribute, get_modifier, is_modifier, Modifier};
use ast::type_resolver::TypeResolver;
use crate::literal::parse_ident;
use crate::parser::ParseInfo;
use crate::util::{parse_code_block, parse_fields};

pub fn parse_top_elements(type_manager: &mut dyn TypeResolver,
                          name: &String, parsing: &mut ParseInfo, parse_code: bool) {
    while let Some(_) = parsing.next_included() {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = get_modifier(parse_modifiers(parsing).as_slice());
        if parsing.matching("struct") {
            parse_struct(type_manager, name, modifiers, parsing, parse_code);
            continue;
        } else if parsing.matching("fn") || is_modifier(modifiers, Modifier::Operation) {
            match parse_function(type_manager, name, attributes, modifiers, parsing, parse_code) {
                Some(mut function) => {
                    if parse_code {
                        function.finalize(type_manager);
                    }
                    if is_modifier(modifiers, Modifier::Operation) {
                        //Add only the method name.
                        type_manager.add_operation(function.name[name.len() + 2..].to_string(), function.name.clone());
                    }
                    type_manager.add_function(function.name.clone(), function)
                }
                None => {}
            };
            continue;
        }

        //Only error once for a big block of issues.
        if parsing.errors.last().is_none() || parsing.errors.last().unwrap().error != "Unknown element" {
            parsing.create_error("Unknown element".to_string());
        } else {
            parsing.skip_line();
        }
    }
}

fn parse_struct(type_manager: &dyn TypeResolver, name: &String,
                modifiers: u8, parsing: &mut ParseInfo, parse_code: bool) {
    todo!()
}

fn parse_function(type_manager: &dyn TypeResolver, name: &String, attributes: HashMap<String, Attribute>,
                  modifiers: u8, parsing: &mut ParseInfo, parse_code: bool) -> Option<Function> {
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

    let code = if parse_code && !is_modifier(modifiers, Modifier::Internal) &&
        !is_modifier(modifiers, Modifier::Extern) {
        match parse_code_block(type_manager, parsing) {
            Some(code) => code,
            None => return None
        }
    } else {
        parsing.find_end();
        CodeBody::new(Vec::new())
    };

    return Some(Function::new(attributes, modifiers, fields, code, return_type, name));
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

fn parse_attributes(parsing: &mut ParseInfo, global: bool) -> HashMap<String, Attribute> {
    let mut output = HashMap::new();
    while parsing.matching("#") {
        if global {
            todo!()
        } else {
            if !parsing.matching("[") {
                parsing.create_error("Expected attribute!".to_string());
                continue;
            }
            let name = parse_ident(parsing);
            match parsing.next_included() {
                Some(value) => match value {
                    b'(' => {
                        match parsing.parse_to(b')') {
                            Some(value) =>
                                if !parsing.matching("]") {
                                    parsing.create_error("Expected closing brace!".to_string());
                                } else {
                                    output.insert(name, Attribute::new(value));
                                },
                            None => parsing.create_error("Unexpected EOF".to_string())
                        }
                    }
                    b']' => {}
                    val => {
                        println!("Value: {}", val as char);
                        parsing.create_error("Expected value or end of attribute".to_string());
                    }
                }
                None => parsing.create_error("Unexpected EOF".to_string())
            }
        }
    }
    return output;
}