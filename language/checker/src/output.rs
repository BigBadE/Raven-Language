use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::{FinalizedFunction, FunctionData, UnfinalizedFunction};
use syntax::{Attribute, ParsingError, ProcessManager, TraitImplementor};
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
    implementations: HashMap<FinalizedTypes, (FinalizedTypes, Vec<Attribute>, Vec<Arc<FunctionData>>)>,
}

impl TypesChecker {
    pub fn new(runtime: Handle) -> Self {
        return Self {
            runtime,
            generics: HashMap::new(),
            implementations: HashMap::new(),
        };
    }
}

#[async_trait]
impl ProcessManager for TypesChecker {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    async fn verify_func(&self, function: UnfinalizedFunction, resolver: Box<dyn NameResolver>, syntax: &Arc<Mutex<Syntax>>) -> FinalizedFunction {
        return match verify_function(self, resolver, function, syntax).await {
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
        match verify_struct(self, structure, &syntax).await {
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

    async fn add_implementation(&mut self, syntax: &Arc<Mutex<Syntax>>, implementor: TraitImplementor) -> Result<(), ParsingError> {
        self.implementations.insert(implementor.implementor.await?.finalize(syntax.clone()).await,
                                    (implementor.base.await?.finalize(syntax.clone()).await,
                                     implementor.attributes, implementor.functions));
        return Ok(());
    }

    /// Checks if the given target type matches the base type.
    /// Can false negative unless parsing is finished.
    async fn of_types(&self, base: &FinalizedTypes, target: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>) -> Option<&Vec<Arc<FunctionData>>> {
        for (implementor, (other, _, functions)) in &self.implementations {
            if base.of_type(implementor, syntax).await &&
                other.of_type(target, syntax).await {
                return Some(&functions);
            }
        }

        return None;
    }

    fn cloned(&self) -> Box<dyn ProcessManager> {
        return Box::new(self.clone());
    }
}
