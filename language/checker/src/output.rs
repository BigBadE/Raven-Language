use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::FunctionData;
use syntax::{Attribute, ParsingError, ProcessManager, TraitImplementor};
use syntax::async_util::NameResolver;
use syntax::r#struct::StructData;
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_function::verify_function;
use crate::check_struct::verify_struct;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    pub generics: HashMap<String, Types>,
    implementations: HashMap<Types, (Types, Vec<Attribute>, Vec<Arc<FunctionData>>)>,
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

    async fn verify_func(&self, function: &mut FunctionData, resolver: Box<dyn NameResolver>, syntax: &Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_function(self, resolver, function, syntax).await {
            syntax.lock().unwrap().errors.push(error.clone());
            function.poisoned.push(error);
        }
    }

    async fn verify_struct(&self, structure: &mut StructData, _resolver: Box<dyn NameResolver>, syntax: Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_struct(self, structure,
                                          &syntax).await {
            syntax.lock().unwrap().errors.push(error.clone());
            structure.poisoned.push(error);
        }
    }

    async fn add_implementation(&mut self, implementor: TraitImplementor) -> Result<(), ParsingError> {
        self.implementations.insert(implementor.implementor.await?,
                                    (implementor.base.await?, implementor.attributes, implementor.functions));
        return Ok(());
    }

    /// Checks if the given target type matches the base type.
    /// Can false negative unless parsing is finished.
    async fn of_types(&self, base: &Types, target: &Types, syntax: &Arc<Mutex<Syntax>>) -> Option<&Vec<Arc<FunctionData>>> {
        for (implementor, (other, _, functions)) in &self.implementations {
            if base.of_type(implementor, syntax).await &&
                other.of_type(target, syntax).await {
                return Some(&functions);
            }
        }

        return None;
    }

    fn get_generic(&self, name: &str) -> Option<Types> {
        return self.generics.get(name).map(|inner| inner.clone());
    }

    fn cloned(&self) -> Box<dyn ProcessManager> {
        return Box::new(self.clone());
    }
}
