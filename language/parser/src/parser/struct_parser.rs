use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use syntax::{Attribute, get_modifier, is_modifier, Modifier, ParsingError, ParsingFuture};
use syntax::async_util::UnparsedType;
use syntax::code::{Field, MemberField};
use syntax::function::Function;
use syntax::r#struct::Struct;
use syntax::types::Types;
use crate::parser::function_parser::{get_generics, parse_function};
use crate::parser::top_parser::{parse_attribute, parse_import, parse_modifier};
use crate::parser::util::{add_generics, ParserUtils};
use crate::tokens::tokens::TokenTypes;

pub struct FutureField(pub ParsingFuture<Types>, pub Vec<Attribute>, pub u8, pub String);

pub fn parse_structure(parser_utils: &mut ParserUtils, attributes: Vec<Attribute>, modifiers: Vec<Modifier>)
                       -> impl Future<Output=Result<Struct, ParsingError>> {
    let mut member_modifiers = Vec::new();
    let mut member_attributes = Vec::new();

    let mut name = String::new();
    let mut fields = Vec::new();
    let mut generics = HashMap::new();
    let mut functions = Vec::new();
    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => name = token.to_string(parser_utils.buffer),
            TokenTypes::GenericsStart => parse_generics(parser_utils, &mut generics),
            TokenTypes::StructTopElement | TokenTypes::Comment => {}
            TokenTypes::InvalidCharacters => parser_utils.syntax.lock().unwrap()
                .add_poison(Arc::new(Struct::new_poisoned(format!("{}", parser_utils.file),
                                                          token.make_error(parser_utils.file.clone(),
                                                                           "Unexpected top element!".to_string())))),
            TokenTypes::ImportStart => parse_import(parser_utils),
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut member_attributes),
            TokenTypes::ModifiersStart => parse_modifier(parser_utils, &mut member_modifiers),
            TokenTypes::FunctionStart => {
                functions.push(parse_function(parser_utils, member_attributes, member_modifiers));
                member_attributes = Vec::new();
                member_modifiers = Vec::new();
            }
            TokenTypes::FieldName => {
                fields.push(parse_field(parser_utils, token.to_string(parser_utils.buffer),
                                        member_attributes, member_modifiers));
                member_attributes = Vec::new();
                member_modifiers = Vec::new();
            }
            TokenTypes::StructEnd => break,
            TokenTypes::EOF => break,
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    let modifiers = get_modifier(modifiers.as_slice());
    if !is_modifier(modifiers, Modifier::Internal) {
        name = parser_utils.file.clone() + "::" + name.as_str();
    }
    parser_utils.syntax.lock().unwrap().structures.parsing.push(name.clone());

    return get_struct(attributes,
                      modifiers, fields, generics, functions, name);
}

pub async fn get_struct(attributes: Vec<Attribute>, modifiers: u8, fields: Vec<FutureField>,
                        generics: HashMap<String, Vec<ParsingFuture<Types>>>,
                        functions: Vec<impl Future<Output=Result<Function, ParsingError>>>, name: String) -> Result<Struct, ParsingError> {
    let generics = get_generics(generics).await?;
    let mut done_fields = Vec::new();
    for field in fields {
        done_fields.push(MemberField::new(field.2, field.1, Field::new(field.3, field.0.await?)))
    }
    let mut done_functions = Vec::new();
    for function in functions {
        let func = Arc::new(function.await?);
        done_functions.push(func);
    }
    return Ok(Struct::new(attributes, done_fields, generics, done_functions, modifiers, name));
}

pub fn parse_implementor(_parser_util: &mut ParserUtils, _attributes: Vec<Attribute>, _modifiers: Vec<Modifier>)
                         -> (Types, Types) {
    todo!()
}

pub fn parse_generics(parser_utils: &mut ParserUtils, generics: &mut HashMap<String, Vec<ParsingFuture<Types>>>) {
    let mut name = String::new();
    let mut unfinished_bounds = Vec::new();
    let mut bounds = Vec::new();
    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Generic => {
                name = token.to_string(parser_utils.buffer);
            }
            TokenTypes::GenericEnd => {
                parser_utils.imports.generics.insert(name.clone(), unfinished_bounds);
                generics.insert(name.clone(), bounds);
                bounds = Vec::new();
                unfinished_bounds = Vec::new();
            }
            TokenTypes::GenericBound => {
                let name = token.to_string(parser_utils.buffer);
                bounds.push(parser_utils.get_struct(token, name.clone()));
                unfinished_bounds.push(add_generics(UnparsedType::Basic(name), parser_utils))
            }
            _ => {
                parser_utils.index -= 1;
                return;
            }
        }
    }
}

pub fn parse_field(parser_utils: &mut ParserUtils, name: String,
                   attributes: Vec<Attribute>, modifiers: Vec<Modifier>) -> FutureField {
    let mut types = None;
    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::FieldType => {
                let name = token.to_string(parser_utils.buffer).clone();
                types = Some(parser_utils.get_struct(token, name))
            },
            TokenTypes::FieldSeparator => {},
            TokenTypes::FieldEnd => break,
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return FutureField(types.unwrap(), attributes, get_modifier(modifiers.as_slice()), name);
}