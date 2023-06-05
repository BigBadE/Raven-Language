use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;
use async_recursion::async_recursion;

use syntax::function::Function;
use syntax::{ParsingError, ParsingFuture, TopElement, TraitImplementor};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::r#struct::Struct;
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

    pub async fn add_struct(syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, token: Token, file: String,
                            structure: ParsingFuture<Struct>) {
        let structure = Self::get_elem(&file, structure).await;

        Syntax::add(&syntax, resolver.boxed_clone(), token.make_error(file.clone(),
                                                        format!("Duplicate structure {}", structure.name)),
                    structure.clone()).await;
    }

    pub async fn add_implementor(syntax: Arc<Mutex<Syntax>>, implementor: ParsingFuture<TraitImplementor>) {
        let temp = implementor.await;
        match temp {
            Ok(implementor) => {
                //Have to clone this twice to get around mutability restrictions and not keep syntax locked across awaits.
                let process_manager = syntax.lock().unwrap().process_manager.cloned();
                process_manager.cloned().add_implementation(implementor, &syntax).await
            },
            Err(error) => {
                syntax.lock().unwrap().structures.types
                    .insert("$main".to_string(), Arc::new(Struct::new_poisoned(
                        "$main".to_string(), error)));
            }
        }
    }

    pub async fn add_function(syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, file: String, token: Token,
                              function: impl Future<Output=Result<Function, ParsingError>>) {
        let adding = Self::get_elem(&file, function).await;
        Syntax::add(&syntax, resolver,
                    token.make_error(file, format!("Duplicate {}", adding.name())),
                                   adding).await;
    }

    async fn get_elem<T: TopElement>(file: &String, adding: impl Future<Output=Result<T, ParsingError>>)
                                     -> Arc<T> {
        return Arc::new(match adding.await {
            Ok(adding) => adding,
            Err(error) => T::new_poisoned(format!("${}", file), error)
        });
    }
}

pub fn add_generics(input: UnparsedType, parser_utils: &mut ParserUtils) -> UnparsedType {
    let mut generics = Vec::new();
    let mut last = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => last = Some(UnparsedType::Basic(token.to_string(parser_utils.buffer))),
            TokenTypes::Operator => if let Some(types) = last {
                generics.push(inner_generic(types, parser_utils));
                last = None;
            },
            TokenTypes::ArgumentEnd => if let Some(types) = last {
                generics.push(types);
                last = None;
            },
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }
    return UnparsedType::Generic(Box::new(input), generics);
}

fn inner_generic(outer: UnparsedType, parser_utils: &mut ParserUtils) -> UnparsedType {
    let mut values = Vec::new();
    let mut last = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                if let Some(found) = last {
                    values.push(found);
                }
                last = Some(UnparsedType::Basic(token.to_string(parser_utils.buffer)));
            },
            TokenTypes::Operator => if let Some(types) = last {
                values.push(inner_generic(types, parser_utils));
                last = None;
            },
            TokenTypes::ArgumentEnd => if let Some(types) = last {
                generics.push(types);
                last = None;
            },
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }

    return UnparsedType::Generic(Box::new(outer), values);
}