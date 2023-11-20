use crate::parser::function_parser::parse_function;
use crate::parser::top_parser::{parse_attribute, parse_import, parse_modifier};
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::{Token, TokenTypes};
use indexmap::IndexMap;
use std::sync::Arc;
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::code::{Field, MemberField};
use syntax::r#struct::{get_internal, StructData, UnfinalizedStruct};
use syntax::syntax::Syntax;
use syntax::types::Types;
use syntax::{
    get_modifier, is_modifier, Attribute, Modifier, ParsingError, ParsingFuture, TraitImplementor,
};

pub fn parse_structure(
    parser_utils: &mut ParserUtils,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> Result<UnfinalizedStruct, ParsingError> {
    let modifiers = get_modifier(modifiers.as_slice());

    let mut member_modifiers = Vec::default();
    let mut member_attributes = Vec::default();

    let mut name = String::default();
    let mut fields = Vec::default();
    let mut generics = IndexMap::default();
    let mut functions = Vec::default();
    while parser_utils.tokens.len() != parser_utils.index {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        let token: Token = token.clone();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                name = token.to_string(parser_utils.buffer);
                parser_utils.imports.parent = Some(name.clone());
            }
            TokenTypes::GenericsStart => parse_generics(parser_utils, &mut generics),
            TokenTypes::StructTopElement | TokenTypes::Comment => {}
            TokenTypes::InvalidCharacters => {
                parser_utils
                    .syntax
                    .lock()
                    .unwrap()
                    .add_poison(Arc::new(StructData::new_poisoned(
                        format!("{}", parser_utils.file),
                        token.make_error(
                            parser_utils.file.clone(),
                            "Unexpected top element!".to_string(),
                        ),
                    )))
            }
            TokenTypes::ImportStart => parse_import(parser_utils),
            TokenTypes::AttributesStart => parse_attribute(parser_utils, &mut member_attributes),
            TokenTypes::ModifiersStart => {
                parse_modifier(parser_utils, &mut member_modifiers);
                if is_modifier(modifiers, Modifier::Internal) {
                    member_modifiers.push(Modifier::Internal);
                }
            }
            TokenTypes::FunctionStart => {
                let file = parser_utils.file.clone();
                if parser_utils.file.is_empty() {
                    parser_utils.file = format!("{}", name);
                } else {
                    parser_utils.file = format!("{}::{}", parser_utils.file, name);
                }
                let function = parse_function(
                    parser_utils,
                    is_modifier(modifiers, Modifier::Trait),
                    member_attributes,
                    member_modifiers,
                );
                functions.push(ParserUtils::add_function(
                    &parser_utils.syntax,
                    parser_utils.file.clone(),
                    function,
                ));
                parser_utils.file = file;
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

    let data =
        if is_modifier(modifiers, Modifier::Internal) && !is_modifier(modifiers, Modifier::Trait) {
            get_internal(name)
        } else {
            let name = format!("{}::{}", parser_utils.file, name);
            Arc::new(StructData::new(
                attributes,
                functions
                    .iter()
                    .map(|inner| inner.data.clone())
                    .collect::<Vec<_>>(),
                modifiers,
                name,
            ))
        };

    return Ok(UnfinalizedStruct {
        generics,
        fields,
        functions,
        data,
    });
}

pub fn parse_implementor(
    parser_utils: &mut ParserUtils,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> Result<TraitImplementor, ParsingError> {
    let mut base = None;
    let mut implementor = None;
    let mut member_attributes = Vec::default();
    let mut member_modifiers = Vec::default();
    let mut functions = Vec::default();
    let mut generics = IndexMap::default();

    let mut state = 0;
    while parser_utils.tokens.len() != parser_utils.index {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Identifier => {
                let name = token.to_string(parser_utils.buffer);
                let temp = Some(UnparsedType::Basic(name.clone()));
                if state == 0 {
                    base = temp;
                    state = 1;
                } else {
                    let mut temp_name = name.clone();
                    let mut depth = 0;
                    while temp_name.as_bytes()[0] == b'[' {
                        temp_name = temp_name[1..temp_name.len() - 1].to_string();
                        depth += 1;
                    }

                    if generics.contains_key(&temp_name) {
                        parser_utils.imports.parent = Some(name);
                    } else {
                        let mut found = String::default();
                        for _ in 0..depth {
                            found += "[";
                        }

                        found += &temp_name;

                        for _ in 0..depth {
                            found += "]";
                        }

                        parser_utils.imports.parent = Some(found);
                    }
                    implementor = temp;
                }
            }
            TokenTypes::GenericsStart => {
                if state == 0 {
                    parse_generics(parser_utils, &mut generics);
                } else {
                    if state == 1 {
                        let found = UnparsedType::Generic(
                            Box::new(base.unwrap()),
                            parse_type_generics(parser_utils)?,
                        );
                        base = Some(found);
                    } else {
                        let found = UnparsedType::Generic(
                            Box::new(implementor.unwrap()),
                            parse_type_generics(parser_utils)?,
                        );
                        implementor = Some(found);
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
                let file = parser_utils.file.clone();
                if parser_utils.file.is_empty() {
                    parser_utils.file =
                        format!("{}_{}", base.clone().unwrap(), implementor.clone().unwrap());
                } else {
                    parser_utils.file = format!(
                        "{}::{}_{}",
                        parser_utils.file,
                        base.clone().unwrap(),
                        implementor.clone().unwrap()
                    );
                }
                let function =
                    parse_function(parser_utils, false, member_attributes, member_modifiers);
                functions.push(function?);
                parser_utils.file = file;
                member_attributes = Vec::default();
                member_modifiers = Vec::default();
            }
            TokenTypes::StructTopElement => {}
            TokenTypes::StructEnd | TokenTypes::EOF => break,
            _ => panic!(
                "How'd you get here? {} - {:?} ({}, {})",
                parser_utils.file,
                token.token_type,
                state,
                token.to_string(parser_utils.buffer)
            ),
        }
    }

    let token = parser_utils.tokens.get(parser_utils.index - 1).unwrap();

    let base = Box::pin(Syntax::parse_type(
        parser_utils.syntax.clone(),
        token.make_error(parser_utils.file.clone(), format!("Failed to find")),
        parser_utils.imports.boxed_clone(),
        base.unwrap(),
        vec![],
    ));

    let implementor = Box::pin(Syntax::parse_type(
        parser_utils.syntax.clone(),
        token.make_error(parser_utils.file.clone(), format!("Failed to find")),
        parser_utils.imports.boxed_clone(),
        implementor.unwrap(),
        vec![],
    ));

    return Ok(TraitImplementor {
        base,
        generics,
        implementor,
        functions,
        attributes,
    });
}

pub fn parse_type_generics(
    parser_utils: &mut ParserUtils,
) -> Result<Vec<UnparsedType>, ParsingError> {
    let mut current = Vec::default();
    while parser_utils.tokens.len() != parser_utils.index {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::GenericsStart => {
                let temp = current.pop().unwrap();
                current.push(UnparsedType::Generic(
                    Box::new(temp),
                    parse_type_generics(parser_utils)?,
                ));
            }
            TokenTypes::Generic => {
                let name = token.to_string(parser_utils.buffer);
                current.push(UnparsedType::Basic(name));
            }
            TokenTypes::GenericsEnd => {
                break;
            }
            TokenTypes::GenericEnd => {}
            _ => {
                panic!("Unexpected type!");
            }
        }
    }

    return Ok(current);
}

pub fn parse_generics(
    parser_utils: &mut ParserUtils,
    generics: &mut IndexMap<String, Vec<ParsingFuture<Types>>>,
) {
    let mut name = String::default();
    let mut bounds: Vec<ParsingFuture<Types>> = Vec::default();
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
                parser_utils
                    .imports
                    .generics
                    .insert(name.clone(), unparsed_bounds);
                generics.insert(name.clone(), bounds);
                bounds = Vec::default();
                unparsed_bounds = Vec::default();
            }
            TokenTypes::GenericBound => {
                let token = parser_utils.tokens.get(parser_utils.index - 1).unwrap();
                let mut name = token.to_string(parser_utils.buffer);
                if name.starts_with(':') {
                    name = name[1..].to_string();
                }
                let name = name.trim().to_string();
                let unparsed = if let Some(inner) = parse_bounds(name.clone(), parser_utils) {
                    inner
                } else {
                    break;
                };
                unparsed_bounds.push(unparsed.clone());
                bounds.push(Syntax::parse_type(
                    parser_utils.syntax.clone(),
                    parser_utils
                        .tokens
                        .get(parser_utils.index - 1)
                        .unwrap()
                        .make_error(parser_utils.file.clone(), format!("Bounds error!")),
                    parser_utils.imports.boxed_clone(),
                    unparsed,
                    vec![],
                ));
            }
            TokenTypes::GenericsEnd => {
                if !name.is_empty() {
                    parser_utils
                        .imports
                        .generics
                        .insert(name.clone(), unparsed_bounds);
                    generics.insert(name.clone(), bounds);
                }

                break;
            }
            _ => panic!(
                "Unknown token type {:?} - {} ({:?})",
                token.token_type,
                parser_utils.file,
                parser_utils.tokens[parser_utils.index - 8..parser_utils.index]
                    .iter()
                    .map(|inner| format!(
                        "{:?} ({})",
                        &inner.token_type,
                        inner.to_string(parser_utils.buffer)
                    ))
                    .collect::<Vec<_>>()
            ),
        }
    }
}

pub fn parse_bounds(name: String, parser_utils: &mut ParserUtils) -> Option<UnparsedType> {
    if parser_utils.tokens[parser_utils.index].token_type == TokenTypes::GenericsStart {
        parser_utils.index += 1;
    } else {
        return Some(UnparsedType::Basic(name));
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
                let unparsed = if let Some(inner) = parse_bounds(name.clone(), parser_utils) {
                    inner
                } else {
                    break;
                };
                unparsed_bounds.push(unparsed);
            }
            TokenTypes::GenericsStart => {
                if let Some(inner) = parse_bounds(String::default(), parser_utils) {
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

    let unparsed = if unparsed_bounds.is_empty() {
        UnparsedType::Basic(name)
    } else {
        UnparsedType::Generic(Box::new(UnparsedType::Basic(name)), unparsed_bounds)
    };

    return Some(unparsed);
}

pub fn parse_field(
    parser_utils: &mut ParserUtils,
    name: String,
    attributes: Vec<Attribute>,
    modifiers: Vec<Modifier>,
) -> ParsingFuture<MemberField> {
    let mut types = None;
    while !parser_utils.tokens.is_empty() {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::FieldType => {
                let name = token.to_string(parser_utils.buffer).clone();
                types = Some(parser_utils.get_struct(token, name))
            }
            TokenTypes::FieldSeparator => {}
            TokenTypes::FieldEnd => break,
            _ => panic!("How'd you get here? {:?}", token.token_type),
        }
    }

    return Box::pin(to_field(
        types.unwrap(),
        attributes,
        get_modifier(modifiers.as_slice()),
        name,
    ));
}

pub async fn to_field(
    types: ParsingFuture<Types>,
    attributes: Vec<Attribute>,
    modifier: u8,
    name: String,
) -> Result<MemberField, ParsingError> {
    return Ok(MemberField::new(
        modifier,
        attributes,
        Field::new(name, types.await?),
    ));
}
