use std::sync::Arc;

use indexmap::IndexMap;

use data::tokens::{Span, TokenTypes};
use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::program::code::MemberField;
use syntax::program::function::{CodeBody, FunctionData, UnfinalizedFunction};
use syntax::program::syntax::Syntax;
use syntax::program::types::Types;
use syntax::{get_modifier, Attribute, Modifier, ParsingFuture};

use crate::parser::code_parser::parse_code;
use crate::parser::struct_parser::{parse_generics, to_field};
use crate::parser::util::ParserUtils;

/// Parses a function
pub fn parse_function(
    parser_utils: &mut ParserUtils,
    trait_function: bool,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> Result<UnfinalizedFunction, ParsingError> {
    let mut name = String::default();
    let mut span = Span::default();
    let mut fields: Vec<ParsingFuture<MemberField>> = Vec::default();
    let mut code = None;
    let mut return_type = None;

    let mut last_arg = String::default();
    let mut last_arg_type = String::default();

    while !parser_utils.tokens.is_empty() {
        let token = &parser_utils.tokens[parser_utils.index];
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                name = parser_utils.file_name.clone() + "::" + &*token.to_string(parser_utils.buffer);
                span = Span::new(parser_utils.file, parser_utils.index - 1);
            }
            TokenTypes::GenericsStart => parse_generics(parser_utils),
            TokenTypes::ArgumentsStart | TokenTypes::ArgumentSeparator | TokenTypes::ArgumentTypeSeparator => {}
            TokenTypes::ArgumentName => last_arg = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentType => last_arg_type = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentEnd => {
                if last_arg_type.is_empty() {
                    if !parser_utils.imports.parent.is_some() {
                        return Err(
                            Span::new(parser_utils.file, parser_utils.index - 1).make_error(ParsingMessage::SelfInStatic)
                        );
                    }

                    fields.push(Box::pin(to_field(
                        Syntax::parse_type(
                            parser_utils.syntax.clone(),
                            Box::new(parser_utils.imports.clone()),
                            parser_utils.imports.parent.clone().unwrap(),
                            vec![],
                        ),
                        Vec::default(),
                        0,
                        last_arg,
                    )));
                } else {
                    fields.push(Box::pin(to_field(
                        parser_utils.get_struct(&Span::new(parser_utils.file, parser_utils.index - 1), last_arg_type),
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
                return_type = Some(parser_utils.get_struct(&Span::new(parser_utils.file, parser_utils.index - 1), ret_name))
            }
            TokenTypes::CodeStart => {
                let temp = parse_code(parser_utils)?.1;
                code = Some(temp);
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
        modifiers += Modifier::Trait as u8;
    }

    return Ok(UnfinalizedFunction {
        generics: parser_utils.imports.generics.clone(),
        fields,
        code: code.unwrap_or_else(|| CodeBody::new(Vec::default(), "empty".to_string())),
        return_type,
        data: Arc::new(FunctionData::new(attributes, modifiers, name, span.clone())),
        parent: parser_utils.imports.parent.clone().map(|types| {
            Syntax::parse_type(parser_utils.syntax.clone(), Box::new(parser_utils.imports.clone()), types, vec![])
        }),
    });
}

/// Awaits the ParsingFuture for the generics
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
