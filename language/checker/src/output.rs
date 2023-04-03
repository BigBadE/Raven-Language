use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Handle;
use compilers::compiling::Compiler;
use syntax::function::Function;
use syntax::functions::Function;
use syntax::ProcessManager;
use syntax::structures::Structure;
use syntax::types::FinalizedType;

pub struct TypesCompiler {
    runtime: Handle
}

impl TypesCompiler {
    pub fn new(runtime: Handle, compiler: Arc<dyn Compiler>) -> Self {
        return Self {
            runtime
        }
    }
}

impl ProcessManager<FinalizedType> for TypesCompiler {
    fn handle(&self) -> &Handle {
        return &self.runtime;
    }

    fn add_to_next(&mut self, _adding: Arc<Structure<FinalizedType>>) {}

    fn add_func_to_next(&mut self, _adding: Arc<Function<FinalizedType>>) {}
}