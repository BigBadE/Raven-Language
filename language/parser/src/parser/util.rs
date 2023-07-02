use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use syntax::function::{CodeBody, FunctionData, UnfinalizedFunction};
use syntax::{ParsingError, ParsingFuture, TopElement, TraitImplementor};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::r#struct::{StructData, UnfinalizedStruct};
use syntax::syntax::Syntax;
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
                      structure: Result<UnfinalizedStruct, ParsingError>) {
        let structure = match structure {
            Ok(adding) => adding,
            Err(error) => UnfinalizedStruct {
                generics: Default::default(),
                fields: Vec::new(),
                data: Arc::new(StructData::new_poisoned(format!("${}", file), error)),
            }
        };

        Syntax::add(&syntax, handle, resolver.boxed_clone(), token.make_error(file.clone(),
                                                        format!("Duplicate structure {}", structure.data.name)),
                    structure.data.clone(), structure);
    }

    pub async fn add_implementor(syntax: Arc<Mutex<Syntax>>, implementor: Result<TraitImplementor, ParsingError>) {
        match implementor {
            Ok(implementor) => {
                //Have to clone this twice to get around mutability restrictions and not keep syntax locked across awaits.
                let process_manager = syntax.lock().unwrap().process_manager.cloned();
                match process_manager.cloned().add_implementation(&syntax, implementor).await {
                    Ok(_) => {},
                    Err(error) => {
                        syntax.lock().unwrap().errors.push(error);
                    }
                };
            },
            Err(error) => {
                syntax.lock().unwrap().errors.push(error);
            }
        }
    }

    pub fn add_function(syntax: &Arc<Mutex<Syntax>>, handle: &Handle, resolver: Box<dyn NameResolver>, file: String, token: Token,
                        function: Result<UnfinalizedFunction, ParsingError>) -> Arc<FunctionData> {
        let adding = match function {
            Ok(adding) => adding,
            Err(error) => UnfinalizedFunction {
                generics: Default::default(),
                fields: Vec::new(),
                code: Box::pin(empty_code()),
                return_type: None,
                data: Arc::new(FunctionData::new_poisoned(format!("${}", file), error)),
            }
        };
        let data = adding.data.clone();
        Syntax::add(syntax, handle, resolver,
                    token.make_error(file, format!("Duplicate {}", adding.data.name())),
                                   adding.data.clone(), adding);
        return data;
    }
}

async fn empty_code() -> Result<CodeBody, ParsingError> {
    return Ok(CodeBody::new(Vec::new(), String::new()));
}

pub fn add_generics(input: String, parser_utils: &mut ParserUtils) -> (UnparsedType, ParsingFuture<Types>) {
    let mut generics: Vec<ParsingFuture<Types>> = Vec::new();
    let mut unparsed_generics = Vec::new();
    let mut last: Option<(UnparsedType, ParsingFuture<Types>)> = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => last = Some((UnparsedType::Basic(token.to_string(parser_utils.buffer)), Box::pin(Syntax::get_struct(parser_utils.syntax.clone(),
                                                                   token.make_error(parser_utils.file.clone(), format!("")),
                                                                   token.to_string(parser_utils.buffer), Box::new(parser_utils.imports.clone()))))),
            TokenTypes::Operator => if let Some((unparsed, types)) = last {
                let (unparsed, types) = inner_generic(unparsed, types, parser_utils);
                generics.push(Box::pin(types));
                unparsed_generics.push(unparsed);
                last = None;
            },
            TokenTypes::ArgumentEnd => if let Some((unparsed, types)) = last {
                unparsed_generics.push(unparsed);
                generics.push(types);
                last = None;
            },
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }
    return (UnparsedType::Generic(Box::new(UnparsedType::Basic(input.clone())), unparsed_generics),
            Box::pin(to_generic(input, generics)));
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
    let mut last: Option<(UnparsedType, ParsingFuture<Types>)> = None;
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
                             Box::pin(Syntax::get_struct(parser_utils.syntax.clone(),
                                                token.make_error(parser_utils.file.clone(), format!("Idk here")),
                token.to_string(parser_utils.buffer), Box::new(parser_utils.imports.clone())))));
            },
            TokenTypes::Operator => if let Some((unparsed, types)) = last {
                let (unparsed, types) = inner_generic(unparsed, types, parser_utils);
                unparsed_values.push(unparsed);
                values.push(types);
                last = None;
            },
            TokenTypes::ArgumentEnd => if let Some((unparsed, types)) = last {
                unparsed_values.push(unparsed);
                values.push(types);
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