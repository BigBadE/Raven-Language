use std::collections::HashMap;
use std::rc::Rc;
use ast::function::{CodeBody, Function};
use ast::{Attribute, get_modifier, is_modifier, Modifier};
use ast::code::{Field, MemberField};
use ast::r#struct::Struct;
use ast::type_resolver::TypeResolver;
use ast::types::{ResolvableTypes, Types};
use crate::literal::parse_ident;
use crate::parser::ParseInfo;
use crate::util::{find_if_first, parse_code_block, parse_fields, parse_generics};

pub fn parse_top_elements(type_manager: &mut dyn TypeResolver,
                          name: &String, parsing: &mut ParseInfo) {
    while let Some(_) = parsing.next_included() {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = get_modifier(parse_modifiers(parsing).as_slice());
        if parsing.matching("struct") {
            match parse_struct_type(type_manager, name, modifiers, parsing) {
                Some(struct_type) => type_manager.add_type(Rc::new(struct_type)),
                None => {}
            };
            continue;
        } else if parsing.matching("fn") || is_modifier(modifiers, Modifier::Operation) {
            match parse_function(type_manager, None, name, attributes, modifiers, parsing) {
                Some(function) => {
                    if is_modifier(modifiers, Modifier::Operation) {
                        //Add only the method name.
                        type_manager.add_operation(function.name[name.len() + 2..].to_string(), function.name.clone());
                    }
                    type_manager.add_function(function)
                }
                None => {}
            };
            continue;
        }

        //Only error once for a big block of issues.
        if parsing.errors.last().is_none() || !parsing.errors.last().unwrap().error.starts_with("Unknown element") {
            let mut temp = parsing.clone();
            temp.skip_line();
            parsing.create_error(format!("Unknown element: {}",
                                         String::from_utf8_lossy(&parsing.buffer[parsing.index..temp.index])));
        } else {
            parsing.skip_line();
        }
    }
}

fn parse_struct_type(type_manager: &mut dyn TypeResolver, name: &String,
                     modifiers: u8, parsing: &mut ParseInfo) -> Option<Types> {
    let mut fn_name;
    let mut generics = HashMap::new();
    if let Some(temp_name) = find_if_first(parsing, b'<', b'{') {
        fn_name = temp_name;

        parsing.matching("<");
        parse_generics(parsing, &mut generics);
    } else {
        fn_name = match parsing.parse_to(b'{') {
            Some(name) => name.clone(),
            None => {
                parsing.create_error("Expected string name".to_string());
                return None;
            }
        };
    }

    if !is_modifier(modifiers, Modifier::Internal) {
        fn_name = name.clone() + "::" + fn_name.as_str();
    }

    let mut functions = Vec::new();
    let mut fields = Vec::new();
    while match parsing.next_included() {
        Some(character) => character != b'}',
        None => {
            parsing.create_error("Unexpected EOF before end of struct!".to_string());
            return None;
        }
    } {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = parse_modifiers(parsing);

        if parsing.matching("fn") {
            match parse_function(type_manager, Some(fn_name.clone()), &fn_name, attributes, get_modifier(modifiers.as_slice()),
                                 parsing) {
                Some(mut function) => {
                    if function.fields.iter().any(|field| field.name == "self") {
                        for (key, val) in &generics {
                            function.generics.insert(key.clone(), val.clone());
                        }
                        functions.push(function.name.clone());
                    }
                    type_manager.add_function(function);
                    parsing.index -= 1;
                    continue;
                }
                None => {}
            }
        }

        let field_name = match parsing.parse_to(b':') {
            Some(field_name) => field_name,
            None => {
                parsing.create_error("Expected field name!".to_string());
                return None;
            }
        };


        let field_type = match parsing.parse_to(b';') {
            Some(field_type) => ResolvableTypes::Resolving(field_type),
            None => {
                parsing.create_error("Expected field type!".to_string());
                return None;
            }
        };

        fields.push(MemberField::new(get_modifier(modifiers.as_slice()),
                                     Field::new(field_name, field_type)));
    }

    return Some(Types::new_struct(Struct::new(Some(fields), generics, functions, modifiers, fn_name),
                                  None, Vec::new()));
}

fn parse_function(type_manager: &dyn TypeResolver, parent: Option<String>, name: &String, attributes: HashMap<String, Attribute>,
                  modifiers: u8, parsing: &mut ParseInfo) -> Option<Function> {
    let fn_name;
    let mut generics = HashMap::new();

    if let Some(found_name) = find_if_first(parsing, b'<', b'(') {
        fn_name = name.clone() + "::" + found_name.as_str();

        parse_generics(parsing, &mut generics);

        if parsing.next_included().is_none() {
            panic!("Expected function parameters!");
        }
    } else {
        fn_name = name.clone() + "::" + match parsing.parse_to(b'(') {
            Some(name) => name.clone(),
            None => {
                parsing.create_error("Expected string name".to_string());
                return None;
            }
        }.as_str();
    }

    let fields = match parse_fields(parent, parsing) {
        Some(fields) => fields,
        None => return None
    };

    let return_type = if parsing.matching("->") {
        match parsing.parse_to(b'{') {
            Some(found) => {
                parsing.index -= 1;
                Some(ResolvableTypes::Resolving(found))
            }
            None => {
                parsing.create_error("Expected code body".to_string());
                return None;
            }
        }
    } else {
        None
    };

    let code = if !is_modifier(modifiers, Modifier::Internal) && !is_modifier(modifiers, Modifier::Extern) {
        match parse_code_block(type_manager, parsing) {
            Some(code) => code,
            None => return None
        }
    } else {
        parsing.find_end();
        CodeBody::new(Vec::new())
    };

    return Some(Function::new(attributes, modifiers, fields, generics, code, return_type, fn_name));
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
                    _val => {
                        parsing.create_error("Expected value or end of attribute".to_string());
                    }
                }
                None => parsing.create_error("Unexpected EOF".to_string())
            }
        }
    }
    return output;
}