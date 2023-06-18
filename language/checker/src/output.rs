use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::Function;
use syntax::{Attribute, ProcessManager, TraitImplementor};
use syntax::async_util::NameResolver;
use syntax::r#struct::Struct;
use syntax::syntax::{ParsingType, Syntax};
use syntax::types::Types;
use crate::check_function::verify_function;
use crate::check_struct::verify_struct;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    pub generics: HashMap<String, Types>,
    implementations: HashMap<ParsingType<Types>, (ParsingType<Types>, Vec<Attribute>, Vec<Arc<Function>>)>,
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

    async fn verify_func(&self, function: &mut Function, resolver: Box<dyn NameResolver>, syntax: &Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_function(self, resolver, function, syntax).await {
            syntax.lock().unwrap().errors.push(error.clone());
            function.poisoned.push(error);
        }
    }

    async fn verify_struct(&self, structure: &mut Struct, _resolver: Box<dyn NameResolver>, syntax: Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_struct(self, structure,
                                          &syntax).await {
            syntax.lock().unwrap().errors.push(error.clone());
            structure.poisoned.push(error);
        }
    }

    fn add_implementation(&mut self, implementor: TraitImplementor) {
        self.implementations.insert(implementor.implementor,
                                    (implementor.base, implementor.attributes, implementor.functions));
    }

    async fn of_types(&self, base: &Types, target: &Types, syntax: &Arc<Mutex<Syntax>>) -> Option<&Vec<Arc<Function>>> {
        for (implementor, (other, _, functions)) in &self.implementations {
            if base.of_type(implementor.assume_finished(), syntax).await &&
                other.assume_finished().of_type(target, syntax).await {
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
