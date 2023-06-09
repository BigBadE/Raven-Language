use indexmap::IndexMap;
use syntax::{Attribute, get_modifier, Modifier, ParsingError, ParsingFuture};
use syntax::function::{CodeBody, Function};
use syntax::syntax::ParsingType;
use syntax::types::Types;

use crate::parser::code_parser::parse_code;
use crate::parser::struct_parser::{parse_generics, to_field};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::TokenTypes;

pub fn parse_function(parser_utils: &mut ParserUtils, attributes: Vec<Attribute>, modifiers: Vec<Modifier>)
                      -> Result<Function, ParsingError> {
    let mut name = String::new();
    let mut generics = IndexMap::new();
    let mut fields = Vec::new();
    let mut code = None;
    let mut return_type = None;

    let mut last_arg = String::new();
    let mut last_arg_type = String::new();

    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => name = parser_utils.file.clone() + "::" + &*token.to_string(parser_utils.buffer),
            TokenTypes::GenericsStart => parse_generics(parser_utils, &mut generics),
            TokenTypes::ArgumentsStart | TokenTypes::ArgumentSeparator | TokenTypes::ArgumentTypeSeparator => {}
            TokenTypes::ArgumentName => last_arg = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentType => last_arg_type = token.to_string(parser_utils.buffer),
            TokenTypes::ArgumentEnd => {
                if last_arg_type.is_empty() {
                    if !parser_utils.imports.parent.is_some() {
                        panic!("No parent for {}!", name);
                    }

                    fields.push(ParsingType::new(Box::pin(to_field(parser_utils.get_struct(token,
                                                parser_utils.imports.parent.as_ref().unwrap().clone()),
                        Vec::new(), 0, last_arg))));
                } else {
                    fields.push(ParsingType::new(Box::pin(to_field(parser_utils.get_struct(token, last_arg_type),
                                            Vec::new(), 0, last_arg))));
                    last_arg_type = String::new();
                }
                last_arg = String::new();
            }
            TokenTypes::ArgumentsEnd | TokenTypes::ReturnTypeArrow => {},
            TokenTypes::ReturnType => {
                let name = token.to_string(parser_utils.buffer).clone();
                return_type = Some(ParsingType::new(parser_utils.get_struct(token, name)))
            },
            TokenTypes::CodeStart => {
                println!("Parsing for {}", name);
                code = Some(parse_code(parser_utils).1);
                break
            },
            TokenTypes::CodeEnd => break,
            TokenTypes::EOF => {
                parser_utils.index -= 1;
                break
            }
            TokenTypes::Comment => {},
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
    let modifiers = get_modifier(modifiers.as_slice());
    return Ok(Function::new(attributes, modifiers, fields, generics,
                        code.unwrap_or(Box::pin(const_finished())), return_type, name));
}

async fn const_finished() -> Result<CodeBody, ParsingError> {
    return Ok(CodeBody::new(Vec::new(), String::new()));
}

pub async fn get_generics(generics: IndexMap<String, Vec<ParsingFuture<Types>>>)
    -> Result<IndexMap<String, Types>, ParsingError> {
    let mut done_generics = IndexMap::new();
    for (name, generic) in generics {
        let mut generics = Vec::new();
        for found in generic {
            generics.push(found.await?);
        }
        done_generics.insert(name.clone(), Types::Generic(name, generics));
    }
    return Ok(done_generics);
}