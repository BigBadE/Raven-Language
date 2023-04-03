use std::sync::Arc;
use tokio::runtime::{Handle, Runtime};
use compilers::compiling::Compiler;
use syntax::function::Function;
use syntax::functions::Function;
use crate::imports::ImportManager;
use syntax::ProcessManager;
use syntax::structures::Structure;
use syntax::syntax::Syntax;
use syntax::types::{GenericType, UnresolvedGenericType};
use crate::solidifier::TypeSolidifer;

pub struct TypeResolver {
    pub next: Syntax<GenericType>,
    runtime: Handle,
    imports: ImportManager
}

impl TypeResolver {
    pub fn new(runtime: Handle, compiler: Arc<dyn Compiler>) -> Self {
        return Self {
            next: Syntax::new(Box::new(TypeSolidifer::new(runtime.clone(), compiler))),
            runtime,
            imports: ImportManager::new()
        };
    }
}

impl ProcessManager<UnresolvedGenericType> for TypeResolver {
    fn handle(&self) -> &Handle {
        return self.handle();
    }

    fn add_to_next(&mut self, adding: Arc<Structure<UnresolvedGenericType>>) {
        todo!()
    }

    fn add_func_to_next(&mut self, adding: Arc<Function<UnresolvedGenericType>>) {
        todo!()
    }
}