use std::sync::Arc;
use syntax::{Attribute, Modifier, MODIFIERS};
use syntax::r#struct::Struct;
use crate::parser::function_parser::parse_function;
use crate::parser::struct_parser::{parse_implementor, parse_structure};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::{Token, TokenTypes};

pub fn parse_top(parser_utils: &mut ParserUtils) {
    let mut modifiers = Vec::new();
    let mut attributes = Vec::new();
    while !parser_utils.tokens.is_empty() {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Start => {}
            TokenTypes::InvalidCharacters => parser_utils.syntax.lock().unwrap()
                .add_poison_struct(false, Arc::new(Struct::new_poisoned(format!("${}", parser_utils.file),
                                                                        token.make_error(parser_utils.file.clone(),
                                                                                         "Unexpected top element!".to_string())))),
            TokenTypes::ImportStart => parse_import(parser_utils),
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut attributes),
            TokenTypes::ModifiersStart => parse_modifier(parser_utils, &mut modifiers),
            TokenTypes::FunctionStart => {
                let token = token.clone();
                let function = parse_function(parser_utils, attributes, modifiers);
                parser_utils.syntax.lock().unwrap().remaining += 1;
                parser_utils.handle.spawn(
                    ParserUtils::add_function(parser_utils.syntax.clone(), parser_utils.file.clone(),
                                              token, function));
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::StructStart => {
                let token = token.clone();
                let structure = parse_structure(parser_utils, attributes, modifiers);
                parser_utils.syntax.lock().unwrap().remaining += 1;
                parser_utils.handle.spawn(
                    ParserUtils::add_struct(parser_utils.syntax.clone(), token,
                                            parser_utils.file.clone(), structure));
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::TraitStart => {
                if modifiers.contains(&Modifier::Internal) || modifiers.contains(&Modifier::Extern) {
                    let error = token.make_error(
                        parser_utils.file.clone(), "Traits can't be internal/external!".to_string());
                    drop(parse_structure(parser_utils, attributes, modifiers));
                    parser_utils.syntax.lock().unwrap()
                        .add_poison_struct(false,
                                    Arc::new(Struct::new_poisoned(format!("${}", parser_utils.file),
                                                                  error)));
                    break;
                }
                modifiers.push(Modifier::Trait);
                let token = token.clone();
                let structure = parse_structure(parser_utils, attributes, modifiers);
                parser_utils.handle.spawn(
                    ParserUtils::add_struct(parser_utils.syntax.clone(), token,
                                            parser_utils.file.clone(), structure));
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::ImplStart => {
                let (base, implementor) = parse_implementor(parser_utils, attributes, modifiers);
                parser_utils.syntax.lock().unwrap().process_manager.add_implementation(base, implementor);
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::EOF => return,
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
}

pub fn parse_import(parser_utils: &mut ParserUtils) {
    let next = parser_utils.tokens.get(parser_utils.index).unwrap();
    parser_utils.index += 1;
    let name = next.to_string(parser_utils.buffer);

    match next.token_type {
        TokenTypes::Identifier => {
            parser_utils.imports.imports.insert(name.split("::").last().unwrap().to_string(), name.parse().unwrap());
        }
        _ => {
            parser_utils.index -= 1;
        }
    }

    if parser_utils.tokens.get(parser_utils.index).unwrap().token_type == TokenTypes::ImportEnd {
        parser_utils.index += 1;
    }
}

pub fn parse_attribute(parser_utils: &mut ParserUtils, attributes: &mut Vec<Attribute>) {
    loop {
        let next = parser_utils.tokens.get(parser_utils.index).unwrap();
        if next.token_type != TokenTypes::Attribute {
            return;
        }
        parser_utils.index += 1;
        let string = next.to_string(parser_utils.buffer);
        attributes.push(if string.contains("(") {
            let mut split = string.split("(");
            let name = split.next().unwrap().to_string();
            let value = split.next().unwrap();
            let value = &value[0..value.len()-1];
            match value.parse::<i64>() {
                Ok(value) => Attribute::Integer(name, value),
                Err(_) => match value.parse::<bool>() {
                    Ok(value) => Attribute::Bool(name, value),
                    Err(_) => Attribute::String(name, value.to_string())
                }
            }
        } else {
            Attribute::Basic(string)
        });
    }
}

pub fn parse_modifier(parser_utils: &mut ParserUtils, modifiers: &mut Vec<Modifier>) {
    loop {
        let next = parser_utils.tokens.get(parser_utils.index).unwrap();
        if next.token_type != TokenTypes::Modifier {
            return;
        }
        parser_utils.index += 1;
        let name = next.to_string(parser_utils.buffer);
        modifiers.push(MODIFIERS.iter().find(|modifier| modifier.to_string() == name)
            .expect(format!("Failed to find modifier {} ({}-{})", name, next.start_offset, next.end_offset).as_str()).clone());
    }
}