use std::sync::Arc;
use syntax::{Attribute, Modifier, MODIFIERS, TopElement};
use syntax::async_util::NameResolver;
use syntax::function::FunctionData;
use syntax::r#struct::StructData;
use crate::parser::function_parser::parse_function;
use crate::parser::struct_parser::{parse_implementor, parse_structure};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::{Token, TokenTypes};

pub fn parse_top(parser_utils: &mut ParserUtils) {
    let mut modifiers = Vec::new();
    let mut attributes = Vec::new();
    while parser_utils.tokens.len() != parser_utils.index {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Start | TokenTypes::AttributeEnd => {}
            TokenTypes::InvalidCharacters => parser_utils.syntax.lock().unwrap()
                .add_poison(Arc::new(StructData::new_poisoned(format!("${}", parser_utils.file),
                                                              token.make_error(parser_utils.file.clone(),
                                                                                         "Invalid top element!".to_string())))),
            TokenTypes::ImportStart => parse_import(parser_utils),
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut attributes),
            TokenTypes::ModifiersStart => parse_modifier(parser_utils, &mut modifiers),
            TokenTypes::FunctionStart => {
                let function = parse_function(parser_utils, false, attributes, modifiers);
                let function = ParserUtils::add_function(&parser_utils.syntax, parser_utils.file.clone(), function);
                parser_utils.handle.spawn(FunctionData::verify(function, parser_utils.syntax.clone(),
                                                               Box::new(parser_utils.imports.clone()),
                                                               parser_utils.syntax.lock().unwrap().process_manager.cloned()));

                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::StructStart => {
                let token = token.clone();
                let structure = parse_structure(parser_utils, attributes, modifiers);
                parser_utils.add_struct(token, structure);
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::TraitStart => {
                if modifiers.contains(&Modifier::Internal) || modifiers.contains(&Modifier::Extern) {
                    let error = token.make_error(
                        parser_utils.file.clone(), "Traits can't be internal/external!".to_string());
                    drop(parse_structure(parser_utils, attributes, modifiers));
                    parser_utils.syntax.lock().unwrap()
                        .add_poison(
                            Arc::new(StructData::new_poisoned(format!("${}", parser_utils.file),
                                                              error)));
                    break;
                }
                modifiers.push(Modifier::Trait);
                let token = token.clone();
                let structure = parse_structure(parser_utils, attributes, modifiers);
                parser_utils.add_struct(token, structure);
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::ImplStart => {
                let implementor = parse_implementor(parser_utils,
                                                    attributes, modifiers);
                let process_manager = parser_utils.syntax.lock().unwrap().process_manager.cloned();
                parser_utils.handle.spawn(
                        ParserUtils::add_implementor(parser_utils.syntax.clone(), implementor,
                        parser_utils.imports.boxed_clone(), process_manager));
                attributes = Vec::new();
                modifiers = Vec::new();
            },
            TokenTypes::Comment => {},
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
            parser_utils.imports.imports.push(name);
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
        let next = parser_utils.tokens.get(parser_utils.index+1).unwrap();
        if next.token_type == TokenTypes::AttributeStart {
            parser_utils.index += 1;
            continue
        }
        if next.token_type != TokenTypes::Attribute {
            return;
        }
        parser_utils.index += 2;
        let string = next.to_string(parser_utils.buffer);
        attributes.push(if string.contains("(") {
            let mut split = string.split("(");
            let mut name = split.next().unwrap().to_string().to_lowercase();
            if name.starts_with("#[") {
                name = name[2..].to_string();
            }
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