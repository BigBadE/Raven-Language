use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::check_function::{verify_function, verify_function_code};
use crate::check_struct::verify_struct;
use syntax::async_util::{HandleWrapper, NameResolver};
use syntax::function::{
    CodeBody, CodelessFinalizedFunction, FinalizedCodeBody, FinalizedFunction, FunctionData,
    UnfinalizedFunction,
};
use syntax::r#struct::{FinalizedStruct, StructData, UnfinalizedStruct};
use syntax::syntax::Syntax;
use syntax::types::FinalizedTypes;
use syntax::ProcessManager;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Arc<Mutex<HandleWrapper>>,
    pub generics: HashMap<String, FinalizedTypes>,
    include_refs: bool,
}

impl TypesChecker {
    pub fn new(runtime: Arc<Mutex<HandleWrapper>>, include_refs: bool) -> Self {
        return Self {
            runtime,
            generics: HashMap::default(),
            include_refs,
        };
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
        syntax: &Arc<Mutex<Syntax>>,
    ) -> (CodelessFinalizedFunction, CodeBody) {
        return match verify_function(function, syntax, self.include_refs).await {
            Ok(output) => output,
            Err(error) => {
                syntax.lock().unwrap().errors.push(error.clone());
                (
                    CodelessFinalizedFunction {
                        generics: IndexMap::default(),
                        arguments: vec![],
                        return_type: None,
                        data: Arc::new(FunctionData::new(Vec::default(), 0, String::default())),
                    },
                    CodeBody::new(Vec::default(), String::default()),
                )
            }
        };
    }

    async fn verify_code(
        &self,
        function: CodelessFinalizedFunction,
        code: CodeBody,
        resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedFunction {
        return match verify_function_code(self, resolver, code, function, syntax).await {
            Ok(output) => output,
            Err(error) => {
                syntax.lock().unwrap().errors.push(error.clone());
                FinalizedFunction {
                    generics: IndexMap::default(),
                    fields: vec![],
                    code: FinalizedCodeBody::default(),
                    return_type: None,
                    data: Arc::new(FunctionData::new(Vec::default(), 0, String::default())),
                }
            }
        };
    }

    async fn verify_struct(
        &self,
        structure: UnfinalizedStruct,
        _resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedStruct {
        match verify_struct(self, structure, &syntax, self.include_refs).await {
            Ok(output) => return output,
            Err(error) => {
                syntax.lock().unwrap().errors.push(error.clone());
                FinalizedStruct {
                    generics: IndexMap::default(),
                    fields: vec![],
                    data: Arc::new(StructData::new(
                        Vec::default(),
                        Vec::default(),
                        0,
                        String::default(),
                    )),
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
