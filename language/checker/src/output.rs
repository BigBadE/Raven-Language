use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::{FinalizedFunction, FunctionData, UnfinalizedFunction};
use syntax::ProcessManager;
use syntax::async_util::NameResolver;
use syntax::r#struct::{FinalizedStruct, StructData, UnfinalizedStruct};
use syntax::syntax::Syntax;
use syntax::types::FinalizedTypes;
use crate::check_function::verify_function;
use crate::check_struct::verify_struct;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    pub generics: HashMap<String, FinalizedTypes>,
    include_refs: bool
}

impl TypesChecker {
    pub fn new(runtime: Handle, include_refs: bool) -> Self {
        return Self {
            runtime,
            generics: HashMap::new(),
            include_refs
        };
    }
}

#[async_trait]
impl ProcessManager for TypesChecker {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    async fn verify_func(&self, function: UnfinalizedFunction, resolver: Box<dyn NameResolver>, syntax: &Arc<Mutex<Syntax>>) -> FinalizedFunction {
        return match verify_function(self, resolver, function, syntax, self.include_refs).await {
            Ok(output) => output,
            Err(error) => {
                println!("Error: {}", error);
                syntax.lock().unwrap().errors.push(error.clone());
                FinalizedFunction {
                    generics: Default::default(),
                    fields: vec![],
                    code: Default::default(),
                    return_type: None,
                    data: Arc::new(FunctionData::new(Vec::new(), 0, String::new())),
                }
            }
        }
    }

    async fn verify_struct(&self, structure: UnfinalizedStruct, _resolver: Box<dyn NameResolver>, syntax: Arc<Mutex<Syntax>>) -> FinalizedStruct {
        match verify_struct(self, structure, &syntax, self.include_refs).await {
            Ok(output) => return output,
            Err(error) => {
                syntax.lock().unwrap().errors.push(error.clone());
                FinalizedStruct {
                    generics: Default::default(),
                    fields: vec![],
                    data: Arc::new(StructData::new(Vec::new(), 0, String::new())),
                }
            }
        }
    }

    fn cloned(&self) -> Box<dyn ProcessManager> {
        return Box::new(self.clone());
    }
}
