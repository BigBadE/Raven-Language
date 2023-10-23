#![feature(get_mut_unchecked, box_into_inner)]

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;

use inkwell::context::Context;
use async_trait::async_trait;
use syntax::function::FinalizedFunction;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::{Compiler, Syntax};

use crate::compiler::CompilerImpl;
use crate::type_getter::CompilerTypeGetter;

pub mod internal;
pub mod lifetimes;
pub mod compiler;
pub mod function_compiler;
pub mod main_future;
pub mod type_getter;
pub mod util;
pub mod vtable_manager;

pub struct LLVMCompiler {
    compiling: Arc<RwLock<HashMap<String, Arc<FinalizedFunction>>>>,
    struct_compiling: Arc<RwLock<HashMap<String, Arc<FinalizedStruct>>>>,
    context: Context,
}

unsafe impl Sync for LLVMCompiler {

}

unsafe impl Send for LLVMCompiler {

}

impl LLVMCompiler {
    pub fn new(compiling: Arc<RwLock<HashMap<String, Arc<FinalizedFunction>>>>,
               struct_compiling: Arc<RwLock<HashMap<String, Arc<FinalizedStruct>>>>) -> Self {
        return Self {
            compiling,
            struct_compiling,
            context: Context::create(),
        };
    }
}

#[async_trait]
impl<T> Compiler<T> for LLVMCompiler {
    async fn compile(&self, target: String, syntax: &Arc<Mutex<Syntax>>) -> Option<T> {
        let mut binding = CompilerTypeGetter::new(
            Rc::new(CompilerImpl::new(&self.context)), syntax.clone());

        let result = binding.compile(target, syntax, &self.compiling,
                                     &self.struct_compiling).await;

        return result.map(|inner| unsafe { inner.call() });
    }
}