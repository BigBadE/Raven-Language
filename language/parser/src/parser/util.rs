use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use syntax::function::Function;
use syntax::{is_modifier, Modifier, ParsingError, ParsingFuture, TopElement, TraitImplementor};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::r#struct::Struct;
use syntax::syntax::{ParsingType, Syntax};
use syntax::types::Types;

use crate::{ImportNameResolver, TokenTypes};
use crate::tokens::tokens::Token;

pub struct ParserUtils<'a> {
    pub buffer: &'a [u8],
    pub index: usize,
    pub tokens: Vec<Token>,
    pub syntax: Arc<Mutex<Syntax>>,
    pub file: String,
    pub imports: ImportNameResolver,
    pub handle: Handle,
}

impl<'a> ParserUtils<'a> {
    pub fn get_struct(&self, token: &Token, name: String) -> ParsingFuture<Types> {
        if name.is_empty() {
            panic!("Empty name!");
        }

        return Box::pin(Syntax::get_struct(
            self.syntax.clone(), token.make_error(self.file.clone(),
                                                  format!("Failed to find type named {}", &name)),
            name, Box::new(self.imports.clone())));
    }

    pub fn add_struct(syntax: &Arc<Mutex<Syntax>>, handle: &Handle, resolver: Box<dyn NameResolver>, token: Token, file: String,
                            structure: Result<Struct, ParsingError>) -> Arc<Struct> {
        let structure = Arc::new(match structure {
            Ok(adding) => adding,
            Err(error) => Struct::new_poisoned(format!("${}", file), error)
        });

        Syntax::add(&syntax, handle, resolver.boxed_clone(), token.make_error(file.clone(),
                                                        format!("Duplicate structure {}", structure.name)),
                    structure.clone());
        return structure;
    }

    pub async fn add_implementor(syntax: Arc<Mutex<Syntax>>, implementor: Result<TraitImplementor, ParsingError>) {
        match implementor {
            Ok(implementor) => {
                //Have to clone this twice to get around mutability restrictions and not keep syntax locked across awaits.
                let process_manager = syntax.lock().unwrap().process_manager.cloned();
                process_manager.cloned().add_implementation(implementor);
            },
            Err(error) => {
                syntax.lock().unwrap().structures.types
                    .insert("$main".to_string(), Arc::new(Struct::new_poisoned(
                        "$main".to_string(), error)));
            }
        }
    }

    pub fn add_function(syntax: &Arc<Mutex<Syntax>>, trait_function: bool, handle: &Handle, resolver: Box<dyn NameResolver>, file: String, token: Token,
                              function: Result<Function, ParsingError>) -> Arc<Function> {
        let adding = Arc::new(match function {
            Ok(mut adding) => {
                if trait_function {
                    if is_modifier(adding.modifiers, Modifier::Internal) || is_modifier(adding.modifiers, Modifier::Extern) {
                        Function::new_poisoned(format!("${}", file), token.make_error(
                            file.clone(), "Traits can't be internal/external!".to_string()))
                    } else {
                        adding.modifiers += Modifier::Trait as u8;
                        adding
                    }
                } else {
                    adding
                }
            },
            Err(error) => Function::new_poisoned(format!("${}", file), error)
        });
        Syntax::add(syntax, handle, resolver,
                    token.make_error(file, format!("Duplicate {}", adding.name())),
                                   adding.clone());
        return adding;
    }
}

pub fn add_generics(input: String, parser_utils: &mut ParserUtils) -> (UnparsedType, ParsingType<Types>) {
    let mut generics: Vec<ParsingFuture<Types>> = Vec::new();
    let mut unparsed_generics = Vec::new();
    let mut last = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => last = Some((UnparsedType::Basic(token.to_string(parser_utils.buffer)), Syntax::get_struct(parser_utils.syntax.clone(),
                                                                   token.make_error(parser_utils.file.clone(), format!("")),
                                                                   token.to_string(parser_utils.buffer), Box::new(parser_utils.imports.clone())))),
            TokenTypes::Operator => if let Some((unparsed, types)) = last {
                let (unparsed, types) = inner_generic(unparsed, Box::pin(types), parser_utils);
                generics.push(Box::pin(types));
                unparsed_generics.push(unparsed);
                last = None;
            },
            TokenTypes::ArgumentEnd => if let Some((unparsed, types)) = last {
                unparsed_generics.push(unparsed);
                generics.push(Box::pin(types));
                last = None;
            },
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }
    return (UnparsedType::Generic(Box::new(UnparsedType::Basic(input.clone())), unparsed_generics),
            ParsingType::new(Box::pin(to_generic(input, generics))));
}

async fn to_generic(name: String, generics: Vec<ParsingFuture<Types>>) -> Result<Types, ParsingError> {
    let mut output = Vec::new();
    for generic in generics {
        output.push(generic.await?.clone());
    }
    return Ok(Types::Generic(name, output));
}

fn inner_generic(unparsed: UnparsedType, outer: ParsingFuture<Types>, parser_utils: &mut ParserUtils) -> (UnparsedType, ParsingFuture<Types>) {
    let mut values: Vec<ParsingFuture<Types>> = Vec::new();
    let mut unparsed_values = Vec::new();
    let mut last = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                if let Some((unparsed, found)) = last {
                    unparsed_values.push(unparsed);
                    values.push(Box::pin(found));
                }
                last = Some((UnparsedType::Basic(token.to_string(parser_utils.buffer)),
                             Syntax::get_struct(parser_utils.syntax.clone(),
                                                token.make_error(parser_utils.file.clone(), format!("Idk here")),
                token.to_string(parser_utils.buffer), Box::new(parser_utils.imports.clone()))));
            },
            TokenTypes::Operator => if let Some((unparsed, types)) = last {
                let (unparsed, types) = inner_generic(unparsed, Box::pin(types), parser_utils);
                unparsed_values.push(unparsed);
                values.push(types);
                last = None;
            },
            TokenTypes::ArgumentEnd => if let Some((unparsed, types)) = last {
                unparsed_values.push(unparsed);
                values.push(Box::pin(types));
                last = None;
            },
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }

    return (UnparsedType::Generic(Box::new(unparsed), unparsed_values),
            Box::pin(async_to_generic(outer, values)));
}

async fn async_to_generic(outer: ParsingFuture<Types>, bounds: Vec<ParsingFuture<Types>>) -> Result<Types, ParsingError> {
    let mut new_bounds = Vec::new();
    for bound in bounds {
        new_bounds.push(bound.await?);
    }
    return Ok(Types::GenericType(Box::new(outer.await?), new_bounds));
}