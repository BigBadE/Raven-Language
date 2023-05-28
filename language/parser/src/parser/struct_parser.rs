use std::future::Future;
use std::sync::Arc;
use indexmap::IndexMap;
use syntax::{Attribute, get_all_names, get_modifier, is_modifier, Modifier, ParsingError, ParsingFuture, TraitImplementor};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::code::{Field, MemberField};
use syntax::function::Function;
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
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
    let mut generics = IndexMap::new();
    let mut functions = Vec::new();
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                name = token.to_string(parser_utils.buffer);
                parser_utils.imports.parent = Some(name.clone());
            },
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

    for name in get_all_names(&name) {
        parser_utils.syntax.lock().unwrap().structures.parsing.push(name);
    }

    return get_struct(attributes,
                      modifiers, fields, generics, functions, name);
}

pub async fn get_struct(attributes: Vec<Attribute>, modifiers: u8, fields: Vec<FutureField>,
                        generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
                        functions: Vec<impl Future<Output=Result<Function, ParsingError>>>, name: String) -> Result<Struct, ParsingError> {
    let generics = get_generics(generics).await?;
    let mut done_fields = Vec::new();
    //TODO investigate fields deadlocking with circular references
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

pub fn parse_implementor(parser_utils: &mut ParserUtils, attributes: Vec<Attribute>,
                         _modifiers: Vec<Modifier>) -> ParsingFuture<TraitImplementor> {
    let mut base = None;
    let mut implementor = None;
    let mut member_attributes = Vec::new();
    let mut member_modifiers = Vec::new();
    let mut functions = Vec::new();
    let mut generics = IndexMap::new();

    let mut state = 0;
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                let name = token.to_string(parser_utils.buffer);
                parser_utils.imports.parent = Some(name.clone());
                let temp = Some(UnparsedType::Basic(name));
                if state == 0 {
                    base = temp;
                    state = 1;
                } else {
                    implementor = temp;
                }
            },
            TokenTypes::GenericsStart => {
                if state == 0 {
                    parse_generics(parser_utils, &mut generics);
                } else {
                    let found =
                        parse_type_generics(parser_utils);
                    if state == 1 {
                        base = Some(UnparsedType::Generic(Box::new(base.unwrap()),
                                                     found));
                    } else {
                        implementor = Some(UnparsedType::Generic(Box::new(implementor.unwrap()),
                                                            found));
                    }
                }
            },
            TokenTypes::For => state = 2,
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut member_attributes),
            TokenTypes::ModifiersStart => parse_modifier(parser_utils, &mut member_modifiers),
            TokenTypes::FunctionStart => {
                functions.push(parse_function(parser_utils, member_attributes, member_modifiers));
                member_attributes = Vec::new();
                member_modifiers = Vec::new();
            }
            TokenTypes::StructTopElement => {},
            TokenTypes::StructEnd | TokenTypes::EOF => break,
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    let token = parser_utils.tokens.get(parser_utils.index-1).unwrap();

    let base = Box::pin(
        Syntax::parse_type(
            parser_utils.syntax.clone(),
            token.make_error(parser_utils.file.clone(), format!("Failed to find")),
            parser_utils.imports.boxed_clone(), base.unwrap()));

    let implementor = Box::pin(
        Syntax::parse_type(
            parser_utils.syntax.clone(),
            token.make_error(parser_utils.file.clone(), format!("Failed to find")),
            parser_utils.imports.boxed_clone(), implementor.unwrap()));

    return Box::pin(get_implementation(base, implementor, functions, attributes));
}

async fn get_implementation(base: ParsingFuture<Types>, implementor: ParsingFuture<Types>,
                            functions: Vec<impl Future<Output=Result<Function, ParsingError>>>, attributes: Vec<Attribute>)
    -> Result<TraitImplementor, ParsingError> {
    let mut final_funcs = Vec::new();
    for func in functions {
        final_funcs.push(func.await?);
    }
    return Ok(TraitImplementor {
        base: base.await?,
        implementor: implementor.await?,
        functions: final_funcs,
        attributes
    })
}

pub fn parse_type_generics(parser_utils: &mut ParserUtils) -> Vec<UnparsedType> {
    let mut current = Vec::new();
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::GenericsStart => {
                let mut temp = current.pop().unwrap();
                temp = UnparsedType::Generic(Box::new(temp), parse_type_generics(parser_utils));
            }
            TokenTypes::Generic => {
                let name = token.to_string(parser_utils.buffer);
                current.push(UnparsedType::Basic(name));
            }
            TokenTypes::GenericEnd => {
                break
            }
            _ => {
                panic!("Unexpected type!");
            }
        }
    }
    return current;
}

pub fn parse_generics(parser_utils: &mut ParserUtils, generics: &mut IndexMap<String, Vec<ParsingFuture<Types>>>) {
    let mut name = String::new();
    let mut unfinished_bounds = Vec::new();
    let mut bounds = Vec::new();
    while parser_utils.tokens.len() != parser_utils.index {
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