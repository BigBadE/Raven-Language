use std::sync::Arc;
use indexmap::IndexMap;

use tokio::runtime::Handle;

use syntax::function::{CodeBody, FunctionData, UnfinalizedFunction};
use syntax::{DataType, FinishedTraitImplementor, ParsingError, ParsingFuture, ProcessManager, TopElement, TraitImplementor};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::r#struct::{StructData, UnfinalizedStruct};
use syntax::syntax::Syntax;
use syntax::types::Types;

use no_deadlocks::Mutex;

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

    pub fn add_struct(&self, token: Token, structure: Result<UnfinalizedStruct, ParsingError>) {
        let structure = match structure {
            Ok(adding) => adding,
            Err(error) => {
                println!("Error: {}", error);
                UnfinalizedStruct {
                    generics: Default::default(),
                    fields: Vec::new(),
                    functions: Vec::new(),
                    data: Arc::new(StructData::new_poisoned(format!("${}", self.file), error)),
                }
            }
        };

        Syntax::add::<StructData>(&self.syntax, token.make_error(self.file.clone(),
                                                            format!("Duplicate structure {}", structure.data.name)),
                                  structure.data());
        let process_manager = self.syntax.lock().unwrap().process_manager.cloned();
        self.handle.spawn(StructData::verify(structure, self.syntax.clone(),
                                             Box::new(self.imports.clone()), process_manager));
    }

    pub async fn add_implementor(syntax: Arc<Mutex<Syntax>>, implementor: Result<TraitImplementor, ParsingError>,
                                 resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        match implementor {
            Ok(implementor) => {
                syntax.lock().unwrap().async_manager.parsing_impls += 1;

                match Self::add_implementation(syntax.clone(), implementor, resolver, process_manager).await {
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

    async fn add_implementation(syntax: Arc<Mutex<Syntax>>, implementor: TraitImplementor,
                                resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) -> Result<(), ParsingError> {
        println!("Adding!");
        let mut generics = IndexMap::new();
        for (generic, bounds) in implementor.generics {
            let mut final_bounds = Vec::new();
            for bound in bounds {
                final_bounds.push(bound.await?.finalize(syntax.clone()).await);
            }
            generics.insert(generic, final_bounds);
        }

        let target = implementor.base.await?.finalize(syntax.clone()).await;
        let base = implementor.implementor.await?.finalize(syntax.clone()).await;
        let chalk_type = Arc::new(Syntax::make_impldatum(&generics,
                                                         &target, &base));

        let mut functions = Vec::new();
        for function in &implementor.functions {
            functions.push(function.data.clone());
        }

        let output = FinishedTraitImplementor {
            target,
            base,
            attributes: implementor.attributes,
            functions,
            chalk_type,
            generics,
        };

        {
            let mut locked = syntax.lock().unwrap();
            locked.implementations.push(output);
            locked.async_manager.parsing_impls -= 1;
            println!("Done!");
            for waker in &locked.async_manager.impl_waiters {
                waker.wake_by_ref();
            }
            locked.async_manager.impl_waiters.clear();
        }

        for function in implementor.functions {
            FunctionData::verify(function, syntax.clone(), resolver.boxed_clone(),
                                 process_manager.cloned()).await;
        }

        return Ok(());
    }

    pub fn add_function(syntax: &Arc<Mutex<Syntax>>, file: String,
                        function: Result<UnfinalizedFunction, ParsingError>) -> UnfinalizedFunction {
        let adding = match function {
            Ok(adding) => adding,
            Err(error) => {
                println!("Error: {}", error);
                UnfinalizedFunction {
                    generics: Default::default(),
                    fields: Vec::new(),
                    code: CodeBody::new(Vec::new(), "empty".to_string()),
                    return_type: None,
                    data: Arc::new(FunctionData::new_poisoned(format!("${}", file), error)),
                }
            }
        };

        Syntax::add(syntax, ParsingError::empty(), &adding.data);
        return adding;
    }
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