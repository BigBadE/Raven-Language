use indexmap::IndexMap;
use std::sync::Arc;

use syntax::async_util::{HandleWrapper, NameResolver, UnparsedType};
use syntax::function::{CodeBody, FunctionData, UnfinalizedFunction};
use syntax::r#struct::{StructData, UnfinalizedStruct};
use syntax::syntax::Syntax;
use syntax::types::Types;
use syntax::{
    DataType, FinishedTraitImplementor, ParsingError, ParsingFuture, ProcessManager, TopElement, TraitImplementor,
};

use std::sync::Mutex;

use crate::tokens::tokens::Token;
use crate::{ImportNameResolver, TokenTypes};

/// A struct containing the data needed for parsing
pub struct ParserUtils<'a> {
    /// A buffer containing the file data
    pub buffer: &'a [u8],
    /// Index in the tokens
    pub index: usize,
    /// All found tokens
    pub tokens: Vec<Token>,
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// The current file
    pub file: String,
    /// The current imports
    pub imports: ImportNameResolver,
    /// Handle for spawning async tasks
    pub handle: Arc<Mutex<HandleWrapper>>,
}

impl<'a> ParserUtils<'a> {
    /// Returns a future for getting a struct given its name
    pub fn get_struct(&self, token: &Token, name: String) -> ParsingFuture<Types> {
        if name.is_empty() {
            panic!("Empty name!");
        }

        return Box::pin(Syntax::get_struct(
            self.syntax.clone(),
            token.make_error(self.file.clone(), format!("Failed to find type named {}", &name)),
            name,
            Box::new(self.imports.clone()),
            vec![],
        ));
    }

    /// Adds a struct to the syntax
    pub fn add_struct(&mut self, token: Token, structure: Result<UnfinalizedStruct, ParsingError>) {
        let structure = match structure {
            Ok(adding) => adding,
            Err(error) => UnfinalizedStruct {
                generics: IndexMap::default(),
                fields: Vec::default(),
                functions: Vec::default(),
                data: Arc::new(StructData::new_poisoned(format!("${}", self.file), error)),
            },
        };

        Syntax::add::<StructData>(
            &self.syntax,
            token.make_error(self.file.clone(), format!("Duplicate structure {}", structure.data.name)),
            structure.data(),
        );

        let process_manager = self.syntax.lock().unwrap().process_manager.cloned();
        self.handle.lock().unwrap().spawn(
            structure.data.name.clone(),
            StructData::verify(
                self.handle.clone(),
                structure,
                self.syntax.clone(),
                Box::new(self.imports.clone()),
                process_manager,
            ),
        );
    }

    /// Adds an implementor to the syntax, and handles the errors
    pub async fn add_implementor(
        handle: Arc<Mutex<HandleWrapper>>,
        syntax: Arc<Mutex<Syntax>>,
        implementor: Result<TraitImplementor, ParsingError>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) {
        match implementor {
            Ok(implementor) => {
                match Self::add_implementation(handle.clone(), syntax.clone(), implementor, resolver, process_manager).await
                {
                    Ok(_) => {}
                    Err(error) => {
                        let mut locked = syntax.lock().unwrap();
                        locked.async_manager.parsing_impls -= 1;
                        locked.errors.push(error);
                    }
                };
            }
            Err(error) => {
                let mut locked = syntax.lock().unwrap();
                locked.async_manager.parsing_impls -= 1;
                locked.errors.push(error);
            }
        }
        handle.lock().unwrap().finish_task(&"temp".to_string());
    }

    /// Adds an implementor to the syntax
    async fn add_implementation(
        handle: Arc<Mutex<HandleWrapper>>,
        syntax: Arc<Mutex<Syntax>>,
        implementor: TraitImplementor,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) -> Result<(), ParsingError> {
        let mut generics = IndexMap::default();
        for (generic, bounds) in implementor.generics {
            let mut final_bounds = Vec::default();
            for bound in bounds {
                final_bounds.push(bound.await?.finalize(syntax.clone()).await);
            }
            generics.insert(generic, final_bounds);
        }

        let target = implementor.base.await?;
        let base = implementor.implementor.await?;

        let target = target.finalize(syntax.clone()).await;
        let base = base.finalize(syntax.clone()).await;

        let chalk_type = Arc::new(Syntax::make_impldatum(&generics, &target, &base));

        let mut functions = Vec::default();
        for function in &implementor.functions {
            functions.push(function.data.clone());
        }

        let output =
            FinishedTraitImplementor { target, base, attributes: implementor.attributes, functions, chalk_type, generics };

        {
            let mut locked = syntax.lock().unwrap();
            locked.implementations.push(output);

            locked.async_manager.parsing_impls -= 1;
            for waker in &locked.async_manager.impl_waiters {
                waker.wake_by_ref();
            }
            locked.async_manager.impl_waiters.clear();
        }

        for function in implementor.functions {
            handle.lock().unwrap().spawn(
                function.data.name.clone(),
                FunctionData::verify(
                    handle.clone(),
                    function,
                    syntax.clone(),
                    resolver.boxed_clone(),
                    process_manager.cloned(),
                ),
            );
        }

        return Ok(());
    }

    /// Adds a function to the syntax
    pub fn add_function(
        syntax: &Arc<Mutex<Syntax>>,
        file: String,
        function: Result<UnfinalizedFunction, ParsingError>,
    ) -> UnfinalizedFunction {
        let adding = match function {
            Ok(adding) => adding,
            Err(error) => UnfinalizedFunction {
                generics: IndexMap::default(),
                fields: Vec::default(),
                code: CodeBody::new(Vec::default(), "empty".to_string()),
                return_type: None,
                data: Arc::new(FunctionData::new_poisoned(format!("${}", file), error)),
            },
        };

        Syntax::add(
            syntax,
            ParsingError::new(file, (0, 0), 0, (0, 0), 0, format!("Duplicate function {}", adding.data.name)),
            &adding.data,
        );
        return adding;
    }
}

/// Parses generics, returning both its unparsed form (for copying) and a future (for actually getting the generics)
pub fn parse_generics(input: String, parser_utils: &mut ParserUtils) -> (UnparsedType, ParsingFuture<Types>) {
    let mut generics: Vec<ParsingFuture<Types>> = Vec::default();
    let mut unparsed_generics = Vec::default();
    let mut last: Option<(UnparsedType, ParsingFuture<Types>)> = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                last = Some((
                    UnparsedType::Basic(token.to_string(parser_utils.buffer)),
                    Box::pin(Syntax::get_struct(
                        parser_utils.syntax.clone(),
                        token.make_error(parser_utils.file.clone(), format!("")),
                        token.to_string(parser_utils.buffer),
                        Box::new(parser_utils.imports.clone()),
                        vec![],
                    )),
                ))
            }
            TokenTypes::Operator => {
                if let Some((unparsed, types)) = last {
                    let (unparsed, types) = inner_generic(unparsed, types, parser_utils);
                    generics.push(Box::pin(types));
                    unparsed_generics.push(unparsed);
                    last = None;
                }
            }
            TokenTypes::ArgumentEnd => {
                if let Some((unparsed, types)) = last {
                    unparsed_generics.push(unparsed);
                    generics.push(types);
                    last = None;
                }
            }
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }
    return (
        UnparsedType::Generic(Box::new(UnparsedType::Basic(input.clone())), unparsed_generics),
        Box::pin(to_generic(input, generics)),
    );
}

/// Gets the generic type from its name and bounds
async fn to_generic(name: String, bounds: Vec<ParsingFuture<Types>>) -> Result<Types, ParsingError> {
    let mut output = Vec::default();
    for bound in bounds {
        output.push(bound.await?.clone());
    }

    return Ok(Types::Generic(name, output));
}

/// Inner generic parser, for nested generic types
fn inner_generic(
    unparsed: UnparsedType,
    outer: ParsingFuture<Types>,
    parser_utils: &mut ParserUtils,
) -> (UnparsedType, ParsingFuture<Types>) {
    let mut values: Vec<ParsingFuture<Types>> = Vec::default();
    let mut unparsed_values = Vec::default();
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
                last = Some((
                    UnparsedType::Basic(token.to_string(parser_utils.buffer)),
                    Box::pin(Syntax::get_struct(
                        parser_utils.syntax.clone(),
                        token.make_error(parser_utils.file.clone(), format!("Idk here")),
                        token.to_string(parser_utils.buffer),
                        Box::new(parser_utils.imports.clone()),
                        vec![],
                    )),
                ));
            }
            TokenTypes::Operator => {
                if let Some((unparsed, types)) = last {
                    let (unparsed, types) = inner_generic(unparsed, types, parser_utils);
                    unparsed_values.push(unparsed);
                    values.push(types);
                    last = None;
                }
            }
            TokenTypes::ArgumentEnd => {
                if let Some((unparsed, types)) = last {
                    unparsed_values.push(unparsed);
                    values.push(types);
                    last = None;
                }
            }
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }

    return (UnparsedType::Generic(Box::new(unparsed), unparsed_values), Box::pin(async_to_generic(outer, values)));
}

/// Asynchronously gets a generic type from its base and bounds
async fn async_to_generic(outer: ParsingFuture<Types>, bounds: Vec<ParsingFuture<Types>>) -> Result<Types, ParsingError> {
    let mut new_bounds = Vec::default();
    for bound in bounds {
        new_bounds.push(bound.await?);
    }
    return Ok(Types::GenericType(Box::new(outer.await?), new_bounds));
}
