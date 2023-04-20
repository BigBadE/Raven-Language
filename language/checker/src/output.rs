use std::sync::Arc;

use tokio::runtime::Handle;

use syntax::function::Function;
use syntax::ProcessManager;
use syntax::r#struct::Struct;
use syntax::types::Types;

pub struct TypesCompiler {
    runtime: Handle
}

impl TypesCompiler {
    pub fn new(runtime: Handle) -> Self {
        return Self {
            runtime
        }
    }
}

impl ProcessManager for TypesCompiler {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    fn verify_func(&self, _function: Arc<Function>) {
        todo!()
    }

    fn verify_struct(&self, _structure: Arc<Struct>) {
        todo!()
    }

    fn add_implementation(&self, _base: Types, _implementing: Types) {
        todo!()
    }

    fn get_internal(&self, _name: &str) -> Arc<Struct> {
        todo!()
    }
}