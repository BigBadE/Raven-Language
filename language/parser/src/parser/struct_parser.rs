use std::sync::Arc;

use data::tokens::{Span, Token, TokenTypes};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::program::code::{Field, MemberField};
use syntax::program::r#struct::{get_internal, StructData, UnfinalizedStruct};
use syntax::program::syntax::Syntax;
use syntax::program::types::Types;
use syntax::{get_modifier, is_modifier, Attribute, Modifier, ParsingFuture, TraitImplementor};

use crate::parser::function_parser::parse_function;
use crate::parser::top_parser::{parse_attribute, parse_import, parse_modifier};
use crate::parser::util::ParserUtils;

/// Parses a program
pub fn parse_structure(
    parser_utils: &mut ParserUtils,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> Result<UnfinalizedStruct, ParsingError> {
    let modifiers = get_modifier(modifiers.as_slice());

    let mut member_modifiers = Vec::default();
    let mut member_attributes = Vec::default();

    let start = Span::new(parser_utils.file, parser_utils.index);
    let mut name = String::default();
    let mut fields = Vec::default();
    let mut functions = Vec::default();
    while parser_utils.tokens.len() != parser_utils.index {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        let token: Token = token.clone();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                name = token.to_string(parser_utils.buffer);
                parser_utils.imports.parent =
                    Some(UnparsedType::Basic(Span::new(parser_utils.file, parser_utils.index - 1), name.clone()));
            }
            TokenTypes::GenericsStart => {
                parse_generics(parser_utils); 
                parser_utils.imports.parent = Some(UnparsedType::Generic(
                    Box::new(parser_utils.imports.parent.clone().unwrap()),
                    parser_utils
                        .imports
                        .generics
                        .keys()
                        .map(|inner| UnparsedType::Basic(Span::default(), inner.clone()))
                        .collect::<Vec<_>>(),
                ));
            }
            TokenTypes::StructTopElement | TokenTypes::Comment => {}
            TokenTypes::InvalidCharacters => parser_utils.syntax.lock().add_poison(Arc::new(StructData::new_poisoned(
                format!("{}", parser_utils.file_name),
                Span::new(parser_utils.file, parser_utils.index).make_error(ParsingMessage::UnexpectedTopElement()),
            ))),
            TokenTypes::ImportStart => parse_import(parser_utils),
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut member_attributes),
            TokenTypes::ModifiersStart => {
                parse_modifier(parser_utils, &mut member_modifiers);
                if is_modifier(modifiers, Modifier::Internal) {
                    member_modifiers.push(Modifier::Internal);
                }
            }
            // TODO remove this, not sure why it's here
            TokenTypes::FunctionStart => {
                let file = parser_utils.file_name.clone();
                if parser_utils.file_name.is_empty() {
                    parser_utils.file_name = format!("{}", name);
                } else {
                    parser_utils.file_name = format!("{}::{}", parser_utils.file_name, name);
                }
                let function = parse_function(
                    parser_utils,
                    is_modifier(modifiers, Modifier::Trait),
                    member_attributes,
                    member_modifiers,
                );
                functions.push(ParserUtils::add_function(&parser_utils.syntax, parser_utils.file_name.clone(), function));
                parser_utils.file_name = file;
                member_attributes = Vec::default();
                member_modifiers = Vec::default();
            }
            TokenTypes::FieldName => {
                fields.push(parse_field(
                    parser_utils,
                    token.to_string(parser_utils.buffer),
                    member_attributes,
                    member_modifiers,
                ));
                member_attributes = Vec::default();
                member_modifiers = Vec::default();
            }
            TokenTypes::StructEnd => break,
            TokenTypes::EOF => break,
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    let generics = parser_utils.imports.generics.clone();
    parser_utils.imports.generics.clear();

    let data = if is_modifier(modifiers, Modifier::Internal) && !is_modifier(modifiers, Modifier::Trait) {
        get_internal(name)
    } else {
        let name = format!("{}::{}", parser_utils.file_name, name);
        Arc::new(StructData::new(
            attributes,
            functions.iter().map(|inner| inner.data.clone()).collect::<Vec<_>>(),
            modifiers,
            start,
            name,
        ))
    };

    return Ok(UnfinalizedStruct { generics, fields, functions, data });
}

/// Parses an implementor
pub fn parse_implementor(
    parser_utils: &mut ParserUtils,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> (Result<TraitImplementor, ParsingError>, String, String) {
    let mut base = None;
    let mut implementor = None;

    let mut member_attributes = Vec::default();
    let mut member_modifiers = Vec::default();
    let mut functions = Vec::default();

    let mut state = 0;
    while parser_utils.tokens.len() != parser_utils.index {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                let name = token.to_string(parser_utils.buffer);
                let temp = Some(UnparsedType::Basic(Span::new(parser_utils.file, parser_utils.index - 1), name.clone()));
                if state == 0 {
                    base = temp;
                    state = 1;
                } else {
                    parser_utils.imports.parent = temp.clone();
                    implementor = temp;
                }
            }
            TokenTypes::GenericsStart => {
                if state == 0 {
                    parse_generics(parser_utils);
                } else {
                    let type_generics = match parse_type_generics(parser_utils) {
                        Ok(generics) => generics,
                        Err(error) => return (Err(error), "error".to_string(), "error".to_string()),
                    };
                    if state == 1 {
                        let found = UnparsedType::Generic(Box::new(base.unwrap()), type_generics);
                        base = Some(found);
                    } else {
                        let found = Some(UnparsedType::Generic(Box::new(implementor.unwrap()), type_generics));
                        parser_utils.imports.parent = found.clone();
                        implementor = found;
                    }
                }
            }
            TokenTypes::For => state = 2,
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut member_attributes),
            TokenTypes::ModifiersStart => {
                parse_modifier(parser_utils, &mut member_modifiers);
                if modifiers.contains(&Modifier::Internal) {
                    member_modifiers.push(Modifier::Internal);
                }
            }
            TokenTypes::FunctionStart => {
                let file = parser_utils.file_name.clone();
                if parser_utils.file_name.is_empty() {
                    parser_utils.file_name = format!("{}_{}", base.as_ref().unwrap(), implementor.as_ref().unwrap());
                } else if let Some(implementor) = implementor.as_ref() {
                    parser_utils.file_name =
                        format!("{}::{}_{}", parser_utils.file_name, base.as_ref().unwrap(), implementor);
                } else {
                    parser_utils.file_name = format!("{}::{}", parser_utils.file_name, base.as_ref().unwrap());
                }
                let function = match parse_function(parser_utils, false, member_attributes, member_modifiers) {
                    Ok(inner) => inner,
                    Err(error) => return (Err(error), "error".to_string(), "error".to_string()),
                };
                functions.push(function);
                parser_utils.file_name = file;
                member_attributes = Vec::default();
                member_modifiers = Vec::default();
            }
            TokenTypes::StructTopElement | TokenTypes::Comment => {}
            TokenTypes::StructEnd | TokenTypes::EOF => break,
            TokenTypes::InvalidCharacters => {
                return (
                    Err(Span::new(parser_utils.file, parser_utils.index - 1)
                        .make_error(ParsingMessage::UnexpectedCharacters())),
                    "error".to_string(),
                    "error".to_string(),
                )
            }
            _ => panic!(
                "How'd you get here? {} - {:?} ({}, {})",
                parser_utils.file_name,
                token.token_type,
                state,
                token.to_string(parser_utils.buffer)
            ),
        }
    }

    let base_future = Box::pin(Syntax::parse_type(
        parser_utils.syntax.clone(),
        parser_utils.imports.boxed_clone(),
        base.clone().unwrap(),
        vec![],
    ));

    let implementor_future = if let Some(implementor) = implementor.clone() {
        Some(Syntax::parse_type(parser_utils.syntax.clone(), parser_utils.imports.boxed_clone(), implementor, vec![]))
    } else {
        None
    };

    let generics = parser_utils.imports.generics.clone();
    parser_utils.imports.generics.clear();

    return (
        Ok(TraitImplementor { base: base_future, generics, implementor: implementor_future, functions, attributes }),
        base.unwrap().to_string(),
        implementor.map(|inner| inner.to_string()).unwrap_or("none".to_string()),
    );
}

/// Parses the generic bounds on a type
pub fn parse_type_generics(parser_utils: &mut ParserUtils) -> Result<Vec<UnparsedType>, ParsingError> {
    let mut current = Vec::default();
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::GenericsStart => {
                let temp = current.pop().unwrap();
                current.push(UnparsedType::Generic(Box::new(temp), parse_type_generics(parser_utils)?));
            }
            TokenTypes::Generic => {
                let name = token.to_string(parser_utils.buffer);
                current.push(UnparsedType::Basic(Span::new(parser_utils.file, parser_utils.index - 1), name));
            }
            TokenTypes::GenericsEnd | TokenTypes::GenericBoundEnd => {
                break;
            }
            TokenTypes::GenericEnd => {}
            _ => {
                panic!("Unexpected type {:?}: {}", token.token_type, token.to_string(parser_utils.buffer));
            }
        }
    }

    return Ok(current);
}

/// Parses the generics and adds them to the generics map
pub fn parse_generics(parser_utils: &mut ParserUtils) {
    let mut name = String::default();
    let mut unparsed_bounds: Vec<UnparsedType> = Vec::default();
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Generic => {
                name = token.to_string(parser_utils.buffer);
                if name.starts_with(',') {
                    name = name[1..].to_string();
                }
                name = name.trim().to_string();
            }
            TokenTypes::GenericEnd => {
                parser_utils.imports.generics.insert(
                    name.clone(),
                    unparsed_bounds,
                );
                unparsed_bounds = Vec::default();
            }
            TokenTypes::GenericBound => {
                let token = parser_utils.tokens.get(parser_utils.index - 1).unwrap();
                let mut name = token.to_string(parser_utils.buffer);
                if name.starts_with(':') {
                    name = name[1..].to_string();
                }
                let name = name.trim().to_string();
                let unparsed = if let Some(inner) = parse_bounds(
                    UnparsedType::Basic(Span::new(parser_utils.file, parser_utils.index - 1), name.clone()),
                    parser_utils,
                ) {
                    inner
                } else {
                    break;
                };
                unparsed_bounds.push(unparsed.clone());
            }
            TokenTypes::GenericsEnd => {
                if !name.is_empty() {
                    parser_utils.imports.generics.insert(
                        name.clone(),
                        unparsed_bounds,
                    );
                }

                break;
            }
            _ => panic!(
                "Unknown token type {:?} - {} ({:?})",
                token.token_type,
                parser_utils.file_name,
                parser_utils.tokens[parser_utils.index - 8..parser_utils.index]
                    .iter()
                    .map(|inner| format!("{:?} ({})", &inner.token_type, inner.to_string(parser_utils.buffer)))
                    .collect::<Vec<_>>()
            ),
        }
    }
}

/// Parses the bounds of a generic
pub fn parse_bounds(name: UnparsedType, parser_utils: &mut ParserUtils) -> Option<UnparsedType> {
    if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::GenericsStart {
        parser_utils.index += 1;
    } else {
        return Some(name);
    }
    let mut unparsed_bounds: Vec<UnparsedType> = Vec::default();
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Generic | TokenTypes::GenericBound => {
                let mut name = token.to_string(parser_utils.buffer);
                if name.starts_with(':') {
                    name = name[1..].to_string();
                }
                name = name.trim().to_string();
                let unparsed = if let Some(inner) = parse_bounds(
                    UnparsedType::Basic(Span::new(parser_utils.file, parser_utils.index - 1), name.clone()),
                    parser_utils,
                ) {
                    inner
                } else {
                    break;
                };
                unparsed_bounds.push(unparsed);
            }
            TokenTypes::GenericsStart => {
                if let Some(inner) = parse_bounds(UnparsedType::Basic(Span::default(), String::default()), parser_utils) {
                    unparsed_bounds.push(inner);
                } else {
                    return None;
                }
            }
            TokenTypes::GenericEnd => {}
            TokenTypes::GenericBoundEnd => break,
            TokenTypes::GenericsEnd => {
                parser_utils.index -= 1;
                break;
            }
            _ => {
                parser_utils.index -= 1;
                return None;
            }
        }
    }

    let unparsed = if unparsed_bounds.is_empty() { name } else { UnparsedType::Generic(Box::new(name), unparsed_bounds) };

    return Some(unparsed);
}

/// Parses a single field
pub fn parse_field(
    parser_utils: &mut ParserUtils,
    name: String,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> ParsingFuture<MemberField> {
    let mut types = None;
    while !parser_utils.tokens.is_empty() {
        let token = &parser_utils.tokens[parser_utils.index];
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::FieldType => {
                let name = token.to_string(parser_utils.buffer).clone();
                types = Some(parser_utils.get_struct(&Span::new(parser_utils.file, parser_utils.index - 1), name))
            }
            TokenTypes::FieldSeparator => {}
            TokenTypes::FieldEnd => break,
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Box::pin(to_field(types.unwrap(), attributes, get_modifier(modifiers.as_slice()), name));
}

/// Waits for the type to finish and converts it to a field
pub async fn to_field(
    types: ParsingFuture<Types>,
    attributes: Vec<Attribute>,
    modifier: u8,
    name: String,
) -> Result<MemberField, ParsingError> {
    return Ok(MemberField::new(modifier, attributes, Field::new(name, types.await?)));
}
