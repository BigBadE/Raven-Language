use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::Function;
use syntax::{Attribute, ProcessManager, TraitImplementor};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_code::placeholder_error;
use crate::check_function::verify_function;
use crate::check_struct::verify_struct;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    syntax: Option<Arc<Mutex<Syntax>>>,
    pub generics: HashMap<String, Types>,
    implementations: HashMap<Types, (Types, Vec<Attribute>, Vec<Arc<Function>>)>,
    waiting_implementations: HashMap<Types, Waker>,
    //Parsing implementations that don't have their type resolved
    parsing_implementations: HashMap<UnparsedType, UnparsedType>,
    //Parsing implementations that do have their type resolved, but not their impl
    finished_parsing_impl: HashMap<Types, Types>,
}

impl TypesChecker {
    pub fn new(runtime: Handle) -> Self {
        return Self {
            runtime,
            syntax: None,
            generics: HashMap::new(),
            implementations: HashMap::new(),
            waiting_implementations: HashMap::new(),
            parsing_implementations: HashMap::new(),
            finished_parsing_impl: HashMap::new(),
        };
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

    async fn add_implementation(&mut self, implementor: TraitImplementor, syntax: &Arc<Mutex<Syntax>>) {
        for waker in &self.waiting_implementations {
            if implementor.base.of_type(&waker.0, syntax).await {
                waker.1.wake_by_ref();
            }
        }
        self.implementations.insert(implementor.implementor,
                                    (implementor.base, implementor.attributes, implementor.functions));
    }

    fn add_impl_parsing(&mut self, base: UnparsedType, target: UnparsedType) {
        self.parsing_implementations.insert(base, target);
    }

    async fn check_parsing(&mut self, input: &Types, target: &Types, syntax: &Arc<Mutex<Syntax>>) -> bool {
        for (base, other_target) in self.parsing_implementations {
            Syntax::parse_type(syntax, placeholder_error("Failed to find type!".to_string()), )
        }
        self.parsing_implementations.clear();

        for (base, other_target) in &self.finished_parsing_impl {
            if input.of_type(base, syntax).await && target.of_type(other_target, syntax).await {
                return true;
            }
        }

        return false;
    }

    fn add_impl_waiter(&mut self, waiter: Waker, base: Types) {
        self.waiting_implementations.insert(base, waiter);
    }

    async fn of_types(&self, base: &Types, target: &Types, syntax: &Arc<Mutex<Syntax>>) -> Option<&Vec<Arc<Function>>> {
        for (implementor, (other, _, functions)) in &self.implementations {
            if base.of_type(&implementor, syntax).await &&
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

    fn init(&mut self, syntax: Arc<Mutex<Syntax>>) {
        self.syntax = Some(syntax);
    }
}
