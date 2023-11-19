use indexmap::IndexMap;
use std::sync::Arc;
use syntax::async_util::NameResolver;
use syntax::code::MemberField;
use syntax::function::{CodeBody, FunctionData, UnfinalizedFunction};
use syntax::syntax::Syntax;
use syntax::types::Types;
use syntax::{get_modifier, is_modifier, Attribute, Modifier, ParsingError, ParsingFuture};

use crate::parser::code_parser::parse_code;
use crate::parser::struct_parser::{parse_generics, to_field};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::TokenTypes;

pub fn parse_function(
    parser_utils: &mut ParserUtils,
    trait_function: bool,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> Result<UnfinalizedFunction, ParsingError> {
    let mut name = String::default();
    let mut generics = IndexMap::default();
    let mut fields: Vec<ParsingFuture<MemberField>> = Vec::default();
    let mut code = None;
    let mut return_type = None;

    let mut last_arg = String::default();
    let mut last_arg_type = String::default();

    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                name = parser_utils.file.clone() + "::" + &*token.to_string(parser_utils.buffer)
            }
            TokenTypes::GenericsStart => parse_generics(parser_utils, &mut generics),
            TokenTypes::ArgumentsStart
            | TokenTypes::ArgumentSeparator
            | TokenTypes::ArgumentTypeSeparator => {}
            TokenTypes::ArgumentName => last_arg = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentType => last_arg_type = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentEnd => {
                if last_arg_type.is_empty() {
                    if !parser_utils.imports.parent.is_some() {
                        panic!("No parent for {}!", name);
                    }

                    fields.push(Box::pin(to_field(
                        parser_utils.get_struct(
                            token,
                            parser_utils.imports.parent.as_ref().unwrap().clone(),
                        ),
                        Vec::default(),
                        0,
                        last_arg,
                    )));
                } else {
                    fields.push(Box::pin(to_field(
                        parser_utils.get_struct(token, last_arg_type),
                        Vec::default(),
                        0,
                        last_arg,
                    )));
                    last_arg_type = String::default();
                }
                last_arg = String::default();
            }
            TokenTypes::ArgumentsEnd | TokenTypes::ReturnTypeArrow => {}
            TokenTypes::ReturnType => {
                let ret_name = token.to_string(parser_utils.buffer).clone();
                return_type = Some(parser_utils.get_struct(token, ret_name))
            }
            TokenTypes::CodeStart => {
                code = Some(parse_code(parser_utils)?.1);
                break;
            }
            TokenTypes::CodeEnd => break,
            TokenTypes::EOF => {
                parser_utils.index -= 1;
                break;
            }
            TokenTypes::Comment => {}
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }
    let mut modifiers = get_modifier(modifiers.as_slice());

    if trait_function {
        if is_modifier(modifiers, Modifier::Internal) || is_modifier(modifiers, Modifier::Extern) {
            return Err(parser_utils
                .tokens
                .get(parser_utils.index - 1)
                .unwrap()
                .make_error(
                    parser_utils.file.clone(),
                    "Traits can't be internal/external!".to_string(),
                ));
        } else {
            modifiers += Modifier::Trait as u8;
        }
    }

    for (key, generic) in &parser_utils.imports.generics {
        let mut bounds = Vec::default();
        for bound in generic {
            bounds.push(Syntax::parse_type(
                parser_utils.syntax.clone(),
                ParsingError::empty(),
                parser_utils.imports.boxed_clone(),
                bound.clone(),
                vec![],
            ));
        }
        generics.insert(key.clone(), bounds);
    }

    return Ok(UnfinalizedFunction {
        generics,
        fields,
        code: code.unwrap_or_else(|| CodeBody::new(Vec::default(), "empty".to_string())),
        return_type,
        data: Arc::new(FunctionData::new(attributes, modifiers, name)),
    });
}

pub async fn get_generics(
    generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
) -> Result<IndexMap<String, Types>, ParsingError> {
    let mut done_generics = IndexMap::default();
    for (name, generic) in generics {
        let mut generics = Vec::default();
        for found in generic {
            generics.push(found.await?);
        }

        done_generics.insert(name.clone(), Types::Generic(name, generics));
    }
    return Ok(done_generics);
}
