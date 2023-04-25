use std::sync::{Arc, Mutex};

use tokio::runtime::Handle;

use async_trait::async_trait;

use syntax::function::Function;
use syntax::{ParsingError, ProcessManager};
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_function::verify_function;

#[derive(Clone)]
pub struct TypesChecker {
    runtime: Handle,
    syntax: Option<Arc<Mutex<Syntax>>>
}

impl TypesChecker {
    pub fn new(runtime: Handle) -> Self {
        return Self {
            runtime,
            syntax: None
        }
    }
}

#[async_trait]
impl ProcessManager for TypesChecker {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    async fn verify_func(&self, function: Arc<Function>) -> Result<(), ParsingError> {
        return verify_function(function, self.syntax.as_ref().unwrap()).await;
    }

    async fn verify_struct(&self, _structure: Arc<Struct>) -> Result<(), ParsingError> {
        //TODO
    }

    fn add_implementation(&self, _base: Types, _implementing: Types) {
        todo!()
    }

    fn get_internal(&self, _name: &str) -> Arc<Struct> {
        todo!()
    }

    fn cloned(&self) -> Box<dyn ProcessManager> {
        return Box::new(self.clone());
    }

    fn init(&mut self, syntax: Arc<Mutex<Syntax>>) {
        self.syntax = Some(syntax);
    }
}