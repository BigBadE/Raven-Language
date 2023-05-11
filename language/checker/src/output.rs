use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::Function;
use syntax::ProcessManager;
use syntax::r#struct::{F64, I64, STR, Struct, U64};
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_function::verify_function;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    syntax: Option<Arc<Mutex<Syntax>>>,
    pub generics: HashMap<String, Types>
}

impl TypesChecker {
    pub fn new(runtime: Handle) -> Self {
        return Self {
            runtime,
            syntax: None,
            generics: HashMap::new()
        }
    }
}

#[async_trait]
impl ProcessManager for TypesChecker {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    async fn verify_func(&self, function: &mut Function, syntax: &Arc<Mutex<Syntax>>) {
        if let Err(error) = verify_function(self, function,
                                            self.syntax.as_ref().unwrap()).await {
            syntax.lock().unwrap().errors.push(error.clone());
            function.poisoned.push(error);
        }
    }

    async fn verify_struct(&self, _structure: &mut Struct, _syntax: &Arc<Mutex<Syntax>>) {
        //TODO
    }

    fn add_implementation(&self, _base: Types, _implementing: Types) {
        todo!()
    }

    fn get_internal(&self, name: &str) -> Arc<Struct> {
        return match name {
            "i64" => I64.clone(),
            "u64" => U64.clone(),
            "f64" => F64.clone(),
            "str" => STR.clone(),
            _ => panic!("Unknown internal {}", name)
        };
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