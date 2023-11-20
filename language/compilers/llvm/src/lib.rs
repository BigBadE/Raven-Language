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

/// The compiler that compiles a syntax
pub mod compiler;
/// Compiles a function to LLVM
pub mod function_compiler;
/// Implementations of internal types
pub mod internal;
/// A future that waits on main to finish verifying
pub mod main_future;
/// Handles translating Raven types into LLVM
pub mod type_getter;
/// Utility functions used in other files
pub mod util;
/// Handles Virtual Tables
pub mod vtable_manager;

/// An LLVM compiler and the data it requires
pub struct LLVMCompiler {
    compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
    struct_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
    arguments: CompilerArguments,
    context: Context,
}

/// SAFETY: LLVMCompiler isn't actually multi-threaded, so this is safe
unsafe impl Sync for LLVMCompiler {}

/// SAFETY: LLVMCompiler isn't actually multi-threaded, so this is safe
unsafe impl Send for LLVMCompiler {}

impl LLVMCompiler {
    /// Creates a new LLVM compiler
    pub fn new(
        compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
        struct_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
        arguments: CompilerArguments,
    ) -> Self {
        return Self { compiling, struct_compiling, arguments, context: Context::create() };
    }
}

#[async_trait]
impl<T> Compiler<T> for LLVMCompiler {
    /// Compiles a syntax, with a receiver that is used to wait for verification before running
    async fn compile(&self, mut receiver: Receiver<()>, syntax: &Arc<Mutex<Syntax>>) -> Option<T> {
        let mut binding = CompilerTypeGetter::new(Arc::new(CompilerImpl::new(&self.context)), syntax.clone());

        if CompilerImpl::compile(&mut binding, &self.arguments, syntax, &self.compiling, &self.struct_compiling).await {
            if receiver.recv().await.is_some() {
                return binding.get_target(&self.arguments.target).map(|inner| unsafe { inner.call() });
            }
        } else {
            receiver.recv().await;
        }

        return None;
    }
}
