use std::collections::HashMap;
use std::future::Future;
use syntax::{Attribute, get_modifier, Modifier, ParsingError};
use syntax::async_util::StructureGetter;
use syntax::code::{Field, MemberField};
use syntax::function::{CodeBody, Function};
use syntax::types::Types;
use crate::parser::code_parser::parse_code;
use crate::parser::struct_parser::{FutureField, parse_generics};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::TokenTypes;

pub fn parse_function(parser_utils: &mut ParserUtils, attributes: Vec<Attribute>, modifiers: Vec<Modifier>)
                      -> impl Future<Output=Result<Function, ParsingError>> {
    let mut name = String::new();
    let mut generics = HashMap::new();
    let mut fields = Vec::new();
    let mut code = None;
    let mut return_type = None;

    let mut last_arg = String::new();
    let mut last_arg_type = String::new();

    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap(); parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => name = token.to_string(parser_utils.buffer),
            TokenTypes::GenericsStart => parse_generics(parser_utils, &mut generics),
            TokenTypes::ArgumentsStart | TokenTypes::ArgumentSeparator | TokenTypes::ArgumentTypeSeparator => {}
            TokenTypes::ArgumentName => last_arg = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentType => last_arg_type = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentEnd => {
                if last_arg_type.is_empty() {
                    if !parser_utils.imports.parent.is_some() {
                        panic!("No parent!");
                    }
                    fields.push(FutureField(
                        parser_utils.get_struct(token, parser_utils.imports.parent.as_ref().unwrap().clone()),
                        Vec::new(), 0, last_arg));
                } else {
                    fields.push(FutureField(parser_utils.get_struct(token, last_arg_type), Vec::new(), 0, last_arg));
                    last_arg_type = String::new();
                }
                last_arg = String::new();
            }
            TokenTypes::ArgumentsEnd | TokenTypes::ReturnTypeArrow => {},
            TokenTypes::ReturnType => {
                let name = token.to_string(parser_utils.buffer).clone();
                return_type = Some(parser_utils.get_struct(token, name))
            },
            TokenTypes::CodeStart => {
                code = Some(parse_code(parser_utils));
                break
            },
            TokenTypes::CodeEnd => break,
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
    return get_function(attributes, get_modifier(modifiers.as_slice()), fields, generics,
                        code, return_type, name);
}

pub async fn get_function(attributes: Vec<Attribute>, modifiers: u8, fields: Vec<FutureField>,
                          generics: HashMap<String, Vec<StructureGetter>>,
                          code: Option<impl Future<Output=Result<CodeBody, ParsingError>>>,
                          return_type: Option<StructureGetter>, name: String) -> Result<Function, ParsingError> {
    let mut done_generics = HashMap::new();
    for (name, generic) in generics {
        let mut generics = Vec::new();
        for found in generic {
            generics.push(found.await?);
        }
        done_generics.insert(name.clone(), Types::Generic(name, generics));
    }
    let return_type = match return_type {
        Some(found) => Some(found.await?),
        None => None
    };
    let mut done_fields = Vec::new();
    for field in fields {
        done_fields.push(MemberField::new(field.2, field.1, Field::new(field.3, field.0.await?)))
    }
    let code = match code {
        Some(found) => found.await?,
        None => CodeBody::new(Vec::new(), "empty_trait".to_string())
    };
    return Ok(Function::new(attributes, modifiers, done_fields, done_generics, code,
                            return_type, name));
}