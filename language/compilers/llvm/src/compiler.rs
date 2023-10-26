use std::collections::HashMap;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use inkwell::module::Module;
use inkwell::OptimizationLevel;
use syntax::syntax::Syntax;
use crate::function_compiler::{compile_block, instance_function};

#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use syntax::async_util::EmptyNameResolver;
use syntax::function::FinalizedFunction;
use syntax::ParsingError;
use syntax::r#struct::FinalizedStruct;
use crate::main_future::MainFuture;
use crate::type_getter::CompilerTypeGetter;

pub struct CompilerImpl<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub execution_engine: ExecutionEngine<'ctx>,
}

/// SAFETY LLVM isn't safe for access across multiple threads, but this module only accesses it from
/// one thread at a time.
unsafe impl Send for CompilerImpl<'_> {

}

/// SAFETY See above
unsafe impl Sync for CompilerImpl<'_> {

}

impl<'ctx> CompilerImpl<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();

        return Self {
            module,
            context,
            builder: context.create_builder(),
            execution_engine,
        };
    }

    pub async fn compile(type_getter: &mut CompilerTypeGetter<'ctx>,
                         target: String, syntax: &Arc<Mutex<Syntax>>,
                         functions: &Arc<RwLock<HashMap<String, Arc<FinalizedFunction>>>>,
                         _structures: &Arc<RwLock<HashMap<String, Arc<FinalizedStruct>>>>) -> bool {
        match Syntax::get_function(syntax.clone(), ParsingError::empty(), target,
                                   Box::new(EmptyNameResolver {}), false).await {
            Ok(_) => {},
            Err(_) => return false
        };

        let function = MainFuture { syntax: syntax.clone() }.await;

        instance_function(Arc::new(function.to_codeless()), type_getter);

        while !type_getter.compiling.is_empty() {
            let (function_type, function) = unsafe {
                Arc::get_mut_unchecked(&mut type_getter.compiling)
            }.remove(0);

            if !function.data.poisoned.is_empty() || function.data.name.is_empty() {
                // The checker handles the poisoned functions
                continue
            }

            let finalized_function;
            {
                let reading = functions.read().unwrap();
                finalized_function = if let Some(found) = reading.get(&function.data.name) {
                    found.clone()
                } else {
                    unsafe {
                        Arc::get_mut_unchecked(&mut type_getter.compiling)
                    }.push((function_type, function));
                    continue
                };
            }
            compile_block(&finalized_function.code, function_type,
                          &mut type_getter.for_function(&finalized_function, function_type), &mut 0);
        }

        //let pass_manager = PassManager::create(&self.compiler.module);

        //unsafe {
        //LLVMWriteBitcodeToFile(self.compiler.module.as_mut_ptr(), c_str("main.bc"));
        //}

        //print_formatted(compiler.module.to_string());
        return true;
    }
}