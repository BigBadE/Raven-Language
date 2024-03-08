use std::sync::Arc;
use std::sync::Mutex;

use indexmap::IndexMap;

use data::tokens::{Span, Token, TokenTypes};
use syntax::async_util::{HandleWrapper, NameResolver, UnparsedType};
use syntax::errors::ParsingError;
use syntax::program::function::{CodeBody, FunctionData, UnfinalizedFunction};
use syntax::program::r#struct::{StructData, UnfinalizedStruct};
use syntax::program::syntax::Syntax;
use syntax::program::types::{FinalizedTypes, Types};
use syntax::{
    FinishedStructImplementor, FinishedTraitImplementor, ParsingFuture, ProcessManager, TopElement, TraitImplementor,
};

use crate::ImportNameResolver;

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
    pub file: u64,
    /// The current file name
    pub file_name: String,
    /// The current imports
    pub imports: ImportNameResolver,
    /// Handle for spawning async tasks
    pub handle: Arc<Mutex<HandleWrapper>>,
}

impl<'a> ParserUtils<'a> {
    /// Returns a future for getting a struct given its name
    pub fn get_struct(&self, span: &Span, name: String) -> ParsingFuture<Types> {
        if name.is_empty() {
            panic!("Empty name!");
        }

        let name = if name == "Self" { self.file_name.clone() } else { name };

        return Box::pin(Syntax::get_struct(
            self.syntax.clone(),
            span.clone(),
            name,
            Box::new(self.imports.clone()),
            vec![],
        ));
    }

    /// Adds a struct to the syntax
    pub fn add_struct(&mut self, structure: Result<UnfinalizedStruct, ParsingError>) {
        let mut structure = structure.unwrap_or_else(|error| UnfinalizedStruct {
            generics: IndexMap::default(),
            fields: Vec::default(),
            functions: Vec::default(),
            data: Arc::new(StructData::new_poisoned(format!("${}", self.file), error)),
        });

        Syntax::add_struct(&self.syntax, &mut structure.data);

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
        implementor_trait: Result<TraitImplementor, ParsingError>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
        base: String,
        implementor: String,
    ) -> Result<(), ParsingError> {
        match implementor_trait {
            Ok(implementor_trait) => {
                match Self::add_implementation(handle.clone(), syntax.clone(), implementor_trait, resolver, process_manager)
                    .await
                {
                    Ok(_) => {}
                    Err(error) => {
                        let mut locked = syntax.lock().unwrap();
                        locked.async_manager.parsing_impls -= 1;
                        return Err(error);
                    }
                };
            }
            Err(error) => {
                let mut locked = syntax.lock().unwrap();
                locked.async_manager.parsing_impls -= 1;
                return Err(error);
            }
        }
        handle.lock().unwrap().finish_task(&format!("{}_{}", base, implementor));
        return Ok(());
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
        let target = target.finalize(syntax.clone()).await;

        let mut functions = Vec::default();
        for function in &implementor.functions {
            functions.push(function.data.clone());
        }

        if let Some(base) = implementor.implementor {
            let base = base.await?;
            let base = base.finalize(syntax.clone()).await;

            let chalk_type = Arc::new(Syntax::make_impldatum(&generics, &target, &base));

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
                locked.implementations.push(Arc::new(output));

                locked.async_manager.parsing_impls -= 1;
                for waker in &locked.async_manager.impl_waiters {
                    waker.wake_by_ref();
                }
                locked.async_manager.impl_waiters.clear();
            }
        } else {
            let output = FinishedStructImplementor { target, attributes: implementor.attributes, functions, generics };

            {
                let mut locked = syntax.lock().unwrap();
                for function in &output.functions {
                    locked.functions.add_type(function.clone());
                }

                let mut target = output.target.clone();
                if let Some((base, _bounds)) = target.inner_generic_type() {
                    target = FinalizedTypes::clone(base);
                }

                locked.struct_implementations.entry(target).or_default().push(Arc::new(output));

                locked.async_manager.parsing_impls -= 1;
                for waker in &locked.async_manager.impl_waiters {
                    waker.wake_by_ref();
                }
                locked.async_manager.impl_waiters.clear();
            }
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
        let mut adding = match function {
            Ok(adding) => adding,
            Err(error) => UnfinalizedFunction {
                generics: IndexMap::default(),
                fields: Vec::default(),
                code: CodeBody::new(Vec::default(), "empty".to_string()),
                return_type: None,
                data: Arc::new(FunctionData::new_poisoned(format!("${}", file), error)),
                parent: None,
            },
        };

        Syntax::add_function(syntax, &mut adding.data);
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
                        Span::default(),
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
                        Span::default(),
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

    let types =
        if unparsed_values.is_empty() { unparsed } else { UnparsedType::Generic(Box::new(unparsed), unparsed_values) };
    return (types, Box::pin(async_to_generic(outer, values)));
}

/// Asynchronously gets a generic type from its base and bounds
async fn async_to_generic(outer: ParsingFuture<Types>, bounds: Vec<ParsingFuture<Types>>) -> Result<Types, ParsingError> {
    let mut new_bounds = Vec::default();
    for bound in bounds {
        new_bounds.push(bound.await?);
    }
    return Ok(Types::GenericType(Box::new(outer.await?), new_bounds));
}
