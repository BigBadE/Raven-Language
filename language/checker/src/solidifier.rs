use std::sync::Arc;
use tokio::runtime::{Handle, Runtime};
use compilers::compiling::Compiler;
use syntax::function::Function;
use syntax::functions::Function;
use syntax::ProcessManager;
use syntax::structures::Structure;
use syntax::syntax::Syntax;
use syntax::types::{FinalizedType, GenericType};
use crate::output::TypesCompiler;

pub struct TypeSolidifer {
    pub next: Syntax<FinalizedType>,
    runtime: Handle
}

impl TypeSolidifer {
    pub fn new(runtime: Handle, compiler: Arc<dyn Compiler>) -> Self {
        return Self {
            next: Syntax::new(Box::new(TypesCompiler::new(runtime.clone(), compiler))),
            runtime
        }
    }
}

impl ProcessManager<GenericType> for TypeSolidifer {
    fn handle(&self) -> &Handle {
        return &self.handle();
    }

    fn add_to_next(&mut self, adding: Arc<Structure<GenericType>>) {
        todo!()
    }

    fn add_func_to_next(&mut self, adding: Arc<Function<GenericType>>) {
        todo!()
    }
}