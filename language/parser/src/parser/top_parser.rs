use std::sync::Arc;
use syntax::{Attribute, Modifier, MODIFIERS};
use syntax::r#struct::Struct;
use crate::parser::function_parser::parse_function;
use crate::parser::struct_parser::{parse_implementor, parse_structure};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::TokenTypes;

pub fn parse_top(parser_utils: &mut ParserUtils) {
    let mut modifiers = Vec::new();
    let mut attributes = Vec::new();
    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.remove(0);
        match token.token_type {
            TokenTypes::Start => {}
            TokenTypes::InvalidCharacters => parser_utils.syntax.lock().unwrap()
                .add_struct(None, Arc::new(Struct::new_poisoned(format!("${}", parser_utils.file),
                                                                token.make_error("Unexpected top element!".to_string())))),
            TokenTypes::ImportStart => parse_import(parser_utils),
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut attributes),
            TokenTypes::ModifiersStart => parse_modifier(parser_utils, &mut modifiers),
            TokenTypes::FunctionStart => {
                let function = parse_function(parser_utils, attributes, modifiers);
                parser_utils.handle.spawn(ParserUtils::add_function(parser_utils.syntax.clone(),
                                                                    parser_utils.file.clone(), token.clone(), function));
                attributes = Vec::new();
                modifiers = Vec::new();
            }
            TokenTypes::StructStart => {
                let structure = parse_structure(parser_utils, attributes, modifiers);
                parser_utils.handle.spawn(
                    ParserUtils::add_struct(parser_utils.syntax.clone(), token, parser_utils.file.clone(),
                                            structure));
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
    let next = parser_utils.tokens.remove(0);
    let name = next.to_string(parser_utils.buffer);

    match next.token_type {
        TokenTypes::Identifier => {
            parser_utils.imports.imports.insert(name.split("::").last().unwrap().to_string(), name.parse().unwrap());
        }
        _ => {
            parser_utils.tokens.insert(0, next);
        }
    }
}

pub fn parse_attribute(parser_utils: &mut ParserUtils, attributes: &mut Vec<Attribute>) {
    loop {
        let next = parser_utils.tokens.remove(0);
        if next.token_type != TokenTypes::Attribute {
            parser_utils.tokens.insert(0, next);
            return;
        }
        attributes.push(Attribute::new(next.to_string(parser_utils.buffer)))
    }
}

pub fn parse_modifier(parser_utils: &mut ParserUtils, modifiers: &mut Vec<Modifier>) {
    loop {
        let next = parser_utils.tokens.remove(0);
        if next.token_type != TokenTypes::Modifier {
            parser_utils.tokens.insert(0, next);
            return;
        }
        let name = next.to_string(parser_utils.buffer);
        modifiers.push(MODIFIERS.iter().find(|modifier| modifier.to_string() == name).unwrap().clone());
    }
}