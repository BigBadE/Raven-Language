#![feature(get_mut_unchecked, box_into_inner)]

use std::sync::Arc;
use std::sync::Mutex;

use dashmap::DashMap;
use inkwell::context::Context;
use tokio::sync::mpsc::Receiver;

use async_trait::async_trait;
use data::CompilerArguments;
use syntax::function::FinalizedFunction;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::{Compiler, Syntax};

use crate::compiler::CompilerImpl;
use crate::type_getter::CompilerTypeGetter;

pub mod compiler;
pub mod function_compiler;
pub mod internal;
pub mod main_future;
pub mod type_getter;
pub mod util;
pub mod vtable_manager;

pub struct LLVMCompiler {
    compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
    struct_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
    arguments: CompilerArguments,
    context: Context,
}

unsafe impl Sync for LLVMCompiler {}

unsafe impl Send for LLVMCompiler {}

impl LLVMCompiler {
    pub fn new(
        compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
        struct_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
        arguments: CompilerArguments,
    ) -> Self {
        return Self {
            compiling,
            struct_compiling,
            arguments,
            context: Context::create(),
        };
    }
}

#[async_trait]
impl<T> Compiler<T> for LLVMCompiler {
    async fn compile(&self, mut receiver: Receiver<()>, syntax: &Arc<Mutex<Syntax>>) -> Option<T> {
        let mut binding =
            CompilerTypeGetter::new(Arc::new(CompilerImpl::new(&self.context)), syntax.clone());

        if CompilerImpl::compile(
            &mut binding,
            &self.arguments,
            syntax,
            &self.compiling,
            &self.struct_compiling,
        )
        .await
        {
            if receiver.recv().await.is_some() {
                return binding
                    .get_target(&self.arguments.target)
                    .map(|inner| unsafe { inner.call() });
            }
        } else {
            receiver.recv().await;
        }

        return None;
    }
}
