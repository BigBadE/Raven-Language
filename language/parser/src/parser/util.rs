use parking_lot::Mutex;
use std::sync::Arc;

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

        let process_manager = self.syntax.lock().process_manager.cloned();
        self.handle.lock().spawn(
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
                        let mut locked = syntax.lock();
                        locked.async_manager.parsing_impls -= 1;
                        return Err(error);
                    }
                };
            }
            Err(error) => {
                let mut locked = syntax.lock();
                locked.async_manager.parsing_impls -= 1;
                return Err(error);
            }
        }
        handle.lock().finish_task(&format!("{}_{}", base, implementor));
        return Ok(());
    }

    /// Adds an implementor to the syntax
    async fn add_implementation(
        handle: Arc<Mutex<HandleWrapper>>,
        syntax: Arc<Mutex<Syntax>>,
        implementor: TraitImplementor,
        mut resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) -> Result<(), ParsingError> {
        let mut generics = IndexMap::default();
        for (generic, bounds) in &implementor.generics {
            resolver.generics_mut().insert(generic.clone(), bounds.clone());
        }

        for (generic, bounds) in implementor.generics {
            let mut output_bounds = vec![];
            for bound in bounds {
                output_bounds.push(
                    Syntax::parse_type(syntax.clone(), resolver.boxed_clone(), bound, vec![])
                        .await?
                        .finalize(syntax.clone())
                        .await,
                );
            }
            generics.insert(generic.clone(), FinalizedTypes::Generic(generic, output_bounds));
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
                let mut locked = syntax.lock();
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
                let mut locked = syntax.lock();
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
            handle.lock().spawn(
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

/// Parses generics, returning both its unparsed form
pub fn parse_generics(input: UnparsedType, parser_utils: &mut ParserUtils) -> UnparsedType {
    let mut unparsed_generics = Vec::default();
    let mut last: Option<UnparsedType> = None;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                if let Some(unparsed) = last {
                    unparsed_generics.push(unparsed);
                }
                last = Some(UnparsedType::Basic(
                    Span::new(parser_utils.file, parser_utils.index - 1),
                    token.to_string(parser_utils.buffer),
                ))
            }
            TokenTypes::Operator => {
                if let Some(unparsed) = last {
                    unparsed_generics.push(parse_generics(unparsed, parser_utils));
                    last = None;
                }
            }
            TokenTypes::ArgumentEnd => {
                if let Some(unparsed) = last {
                    unparsed_generics.push(unparsed);
                    last = None;
                }
            }
            _ => {
                parser_utils.index -= 1;
                break;
            }
        }
    }

    return if unparsed_generics.is_empty() {
        input.clone()
    } else {
        UnparsedType::Generic(Box::new(input.clone()), unparsed_generics)
    };
}
