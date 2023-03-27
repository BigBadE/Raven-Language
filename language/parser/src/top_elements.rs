use std::collections::HashMap;
use std::rc::Rc;
use ast::function::{CodeBody, Function};
use ast::{Attribute, get_modifier, is_modifier, Modifier};
use ast::code::{Field, MemberField};
use ast::r#struct::Struct;
use ast::type_resolver::TypeResolver;
use ast::types::{ResolvableTypes, Types};
use crate::literal::{parse_ident, parse_with_references};
use crate::parser::ParseInfo;
use crate::util::{find_if_first, parse_code_block, parse_fields, parse_generics, parse_generics_vec};

pub fn parse_top_elements(type_manager: &mut dyn TypeResolver,
                          name: &String, parsing: &mut ParseInfo) {
    while let Some(_) = parsing.next_included() {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = get_modifier(parse_modifiers(parsing).as_slice());

        if parsing.matching("import") {
            let importing = parse_with_references(parsing);
            type_manager.add_import(&name, importing.replace(" ", ""));
            if !parsing.matching(";") {
                parsing.create_error("Missing semicolon!".to_string());
            }
        } else if parsing.matching("impl") {
            match parse_impl(type_manager, parsing) {
                Some((original, implementing, functions)) => type_manager.add_unresolved_type(name.clone(), original, implementing, functions),
                None => {}
            };
        } else if parsing.matching("trait") {
            match parse_struct_type(type_manager, name, modifiers, parsing, true) {
                Some(trait_type) => type_manager.add_type(Rc::new(trait_type)),
                None => {}
            };
        } else if parsing.matching("struct") {
            match parse_struct_type(type_manager, name, modifiers, parsing, false) {
                Some(struct_type) => type_manager.add_type(Rc::new(struct_type)),
                None => {}
            };
        } else if parsing.matching("fn") || is_modifier(modifiers, Modifier::Operation) {
            match parse_function(type_manager, None, name, attributes, modifiers, parsing, true) {
                Some(function) => {
                    if is_modifier(modifiers, Modifier::Operation) {
                        //Add only the method name.
                        type_manager.add_operation(function.name[name.len() + 2..].to_string(), function.name.clone());
                    }
                    type_manager.add_function(function)
                }
                None => {}
            };
        } else {
            //Only error once for a big block of issues.
            if parsing.errors.last().is_none() || !parsing.errors.last().unwrap().error.starts_with("Unknown element") {
                let mut temp = parsing.clone();
                temp.skip_line();
                parsing.create_error(format!("Unknown element: {}",
                                             String::from_utf8_lossy(&parsing.buffer[parsing.index..temp.index - 1])));
            } else {
                parsing.skip_line();
            }
        }
    }
}

fn parse_impl(type_manager: &mut dyn TypeResolver, parsing: &mut ParseInfo) -> Option<(String, String, Vec<Function>)> {
    parsing.next_included();
    parsing.index -= 1;
    let implementing = match parsing.parse_to_space() {
        Some(found) => found,
        None => {
            parsing.create_error("Unexpected EOF".to_string());
            return None;
        }
    }.split("<").next().unwrap().to_string();
    if !parsing.matching("for") {
        parsing.create_error("Expected for in impl".to_string());
        return None;
    }
    let base = match parsing.parse_to(b'{')  {
        Some(found) => found,
        None => {
            parsing.create_error("Unexpected EOF".to_string());
            return None;
        }
    }.split("<").next().unwrap().to_string();
    let mut functions = Vec::new();
    while match parsing.next_included() {
        Some(character) => character != b'}',
        None => {
            parsing.create_error("Unexpected EOF before end of impl!".to_string());
            return None;
        }
    } {
        parsing.index -= 1;
        let attributes = parse_attributes(parsing, false);
        let modifiers = parse_modifiers(parsing);

        if parsing.matching("fn") {
            match parse_function(type_manager, Some(implementing.clone()), &implementing, attributes,
                                 get_modifier(modifiers.as_slice()), parsing, true) {
                Some(function) => {
                    functions.push(function);
                    continue;
                }
                None => {}
            }
        }
    }
    return Some((base, implementing, functions));
}

fn parse_struct_type(type_manager: &mut dyn TypeResolver, name: &String,
                     modifiers: u8, parsing: &mut ParseInfo, is_trait: bool) -> Option<Types> {
    let mut fn_name = String::new();
    let mut generics = Vec::new();

    if let Some(temp_name) = find_if_first(parsing, b'<', b'{') {
        fn_name = temp_name;

        parsing.matching("<");
        parse_generics_vec(parsing, &mut generics);
    }

    let mut parent_types = Vec::new();
    if let Some(temp_name) = find_if_first(parsing, b':', b'{') {
        fn_name = temp_name;
        let subtypes = match parsing.parse_to(b'{') {
            Some(found) => found,
            None => {
                parsing.create_error("Expected bracket!".to_string());
                return None;
            }
        };
        let subtypes: Vec<ResolvableTypes> = subtypes.split("+").map(|found| ResolvableTypes::Resolving(found.to_string())).collect();

        if subtypes.len() > 1 && !is_trait {
            parsing.create_error("Can't have multiple supertypes on a structure. Implement traits using the impl keyword!".to_string());
            return None;
        }

        parent_types = subtypes;
    } else if fn_name.is_empty() {
        fn_name = match parsing.parse_to(b'{') {
            Some(name) => name.clone(),
            None => {
                parsing.create_error("Expected bracket!".to_string());
                return None;
            }
        };
    } else {
        parsing.matching("{");
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
                                 parsing, !is_trait) {
                Some(mut function) => {
                    if function.fields.iter().any(|field| field.name == "self") {
                        for (key, val) in &generics {
                            function.generics.insert(key.clone(), val.clone());
                        }
                        functions.push(function.name.clone());
                    }
                    type_manager.add_function(function);
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

    return if is_trait {
        //TODO figure out pointer size
        if (modifiers & Modifier::Trait as u8) != 0 {
            panic!("Traits can't have internal or external modifiers!");
        }
        let modifiers = modifiers + Modifier::Trait as u8;
        Some(Types::new_trait(8, Struct::new(Some(fields), generics, functions, modifiers, fn_name),
                              parent_types))
    } else {
        if !parent_types.is_empty() && !fields.is_empty() {
            panic!("Subtypes can't declare new fields!");
        }
        Some(Types::new_struct(Struct::new(Some(fields), generics, functions, modifiers, fn_name),
                               parent_types.get(0).map(|found| found.clone()), Vec::new()))
    }
}

fn parse_function(type_manager: &dyn TypeResolver, parent: Option<String>, name: &String, attributes: HashMap<String, Attribute>,
                  modifiers: u8, parsing: &mut ParseInfo, parse_body: bool) -> Option<Function> {
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
        if parse_body {
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
            match parsing.parse_to(b';') {
                Some(found) => {
                    parsing.index -= 1;
                    Some(ResolvableTypes::Resolving(found))
                }
                None => {
                    parsing.create_error("Expected no body on trait function".to_string());
                    return None;
                }
            }
        }
    } else {
        None
    };

    if !parse_body {
        if !parsing.matching(";") {
            parsing.create_error("Unexpected body on function!".to_string());
        }
        return Some(Function::new(attributes, modifiers, fields, generics, CodeBody::new(Vec::new()), return_type, fn_name));
    }

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