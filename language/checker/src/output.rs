use async_trait::async_trait;
use data::tokens::Span;
use indexmap::IndexMap;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::check_function::{verify_function, verify_function_code};
use crate::check_struct::verify_struct;
use crate::degeneric::degeneric_function;
use syntax::async_util::{HandleWrapper, NameResolver};
use syntax::program::function::{
    CodeBody, CodelessFinalizedFunction, FinalizedCodeBody, FinalizedFunction, FunctionData, UnfinalizedFunction,
};
use syntax::program::r#struct::{FinalizedStruct, StructData, UnfinalizedStruct};
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;
use syntax::{ProcessManager, SimpleVariableManager};

/// Wrapper around data used by checkers
#[derive(Clone)]
pub struct TypesChecker {
    /// Runtime for ProcessManager
    runtime: Arc<Mutex<HandleWrapper>>,
    /// Generics in the current type
    pub generics: HashMap<String, FinalizedTypes>,
}

impl TypesChecker {
    /// Makes a new TypesChecker
    pub fn new(runtime: Arc<Mutex<HandleWrapper>>) -> Self {
        return Self { runtime, generics: HashMap::default() };
    }
}

#[async_trait]
impl ProcessManager for TypesChecker {
    fn handle(&self) -> &Arc<Mutex<HandleWrapper>> {
        return &self.runtime;
    }

    async fn verify_func(
        &self,
        function: UnfinalizedFunction,
        resolver: &dyn NameResolver,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> (CodelessFinalizedFunction, CodeBody) {
        return verify_function(function, resolver, syntax).await.unwrap_or_else(|error| {
            syntax.lock().errors.push(error.clone());
            (
                CodelessFinalizedFunction {
                    generics: IndexMap::default(),
                    arguments: vec![],
                    return_type: None,
                    data: Arc::new(FunctionData::new(Vec::default(), 0, String::default(), Span::default())),
                    parent: None,
                },
                CodeBody::new(Vec::default(), String::default()),
            )
        });
    }

    async fn verify_code(
        &self,
        function: CodelessFinalizedFunction,
        code: CodeBody,
        resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedFunction {
        let name = function.data.name.clone();
        return verify_function_code(self, resolver, code, function, syntax).await.unwrap_or_else(|error| {
            syntax.lock().errors.push(error.clone());
            FinalizedFunction {
                generics: IndexMap::default(),
                fields: vec![],
                code: FinalizedCodeBody::default(),
                return_type: None,
                data: Arc::new(FunctionData::poisoned(name, error)),
            }
        });
    }

    async fn degeneric_code(&self, function: Arc<CodelessFinalizedFunction>, syntax: &Arc<Mutex<Syntax>>) {
        let variables = SimpleVariableManager::for_function(&function);
        let _ = match degeneric_function(function, Box::new(self.clone()), &vec![], syntax, &variables, vec![]).await {
            Ok(result) => result,
            Err(error) => panic!("{:?}", error),
        };
    }

    async fn verify_struct(
        &self,
        structure: UnfinalizedStruct,
        resolver: &dyn NameResolver,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedStruct {
        match verify_struct(self, structure, resolver, &syntax).await {
            Ok(output) => return output,
            Err(error) => {
                syntax.lock().errors.push(error.clone());
                FinalizedStruct {
                    generics: IndexMap::default(),
                    fields: vec![],
                    data: Arc::new(StructData::new(Vec::default(), Vec::default(), 0, Span::default(), String::default())),
                }
            }
        }
    }

    fn generics(&self) -> &HashMap<String, FinalizedTypes> {
        return &self.generics;
    }

    fn mut_generics(&mut self) -> &mut HashMap<String, FinalizedTypes> {
        return &mut self.generics;
    }

    fn cloned(&self) -> Box<dyn ProcessManager> {
        return Box::new(self.clone());
    }
}
