use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::Function;
use syntax::{Attribute, ProcessManager, TraitImplementor};
use syntax::async_util::NameResolver;
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_function::verify_function;
use crate::check_struct::verify_struct;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    syntax: Option<Arc<Mutex<Syntax>>>,
    pub generics: HashMap<String, Types>,
    implementations: HashMap<Types, (Types, Vec<Attribute>, Vec<Arc<Function>>)>
}

impl TypesChecker {
    pub fn new(runtime: Handle) -> Self {
        return Self {
            runtime,
            syntax: None,
            generics: HashMap::new(),
            implementations: HashMap::new()
        }
    }
}

#[async_trait]
impl ProcessManager for TypesChecker {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    async fn verify_func(&self, function: &mut Function, resolver: Box<dyn NameResolver>, syntax: &Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_function(self, resolver, function,
                                            &self.syntax.clone().unwrap()).await {
            syntax.lock().unwrap().errors.push(error.clone());
            function.poisoned.push(error);
        }
    }

    async fn verify_struct(&self, structure: &mut Struct, _resolver: Box<dyn NameResolver>, syntax: &Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_struct(self, structure,
                                            &self.syntax.clone().unwrap()).await {
            syntax.lock().unwrap().errors.push(error.clone());
            structure.poisoned.push(error);
        }
    }

    fn add_implementation(&mut self, implementor: TraitImplementor) {
        self.implementations.insert(implementor.implementor,
                                    (implementor.base, implementor.attributes, implementor.functions));
    }

    fn of_types(&self, base: &Types, target: &Types) -> Option<&Vec<Arc<Function>>> {
        for (implementor, (other, _, functions)) in &self.implementations {
            if base.of_type(&implementor) && other.of_type(target) {
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

    fn init(&mut self, syntax: Arc<Mutex<Syntax>>) {
        self.syntax = Some(syntax);
    }
}