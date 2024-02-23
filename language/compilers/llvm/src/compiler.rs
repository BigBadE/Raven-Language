use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use inkwell::module::Module;
use inkwell::OptimizationLevel;
use tokio::time;

use data::tokens::Span;
use data::CompilerArguments;
use syntax::async_util::EmptyNameResolver;
use syntax::program::function::{CodelessFinalizedFunction, FinalizedFunction};
use syntax::program::r#struct::FinalizedStruct;
use syntax::program::syntax::Syntax;

use crate::function_compiler::{compile_block, instance_function};
use crate::main_future::MainFuture;
use crate::type_getter::CompilerTypeGetter;

/// A compiler implementation which must wrap the context
pub struct CompilerImpl<'ctx> {
    /// LLVM context
    pub context: &'ctx Context,
    /// LLVM module
    pub module: Module<'ctx>,
    /// LLVM builder
    pub builder: Builder<'ctx>,
    /// LLVM execution engine
    pub execution_engine: ExecutionEngine<'ctx>,
}

impl<'ctx> CompilerImpl<'ctx> {
    /// Creates a new CompilerImpl from the context
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();
        return Self { module, context, builder: context.create_builder(), execution_engine };
    }

    /// Finds the main function
    pub async fn get_main(
        arguments: &CompilerArguments,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> Option<Arc<CodelessFinalizedFunction>> {
        match Syntax::get_function(
            syntax.clone(),
            Span::default(),
            arguments.target.clone(),
            Box::new(EmptyNameResolver {}),
            false,
        )
        .await
        {
            Ok(_) => {}
            Err(_) => return None,
        };

        let function = match time::timeout(Duration::from_secs(5), MainFuture { syntax: syntax.clone() }).await {
            Ok(found) => found,
            Err(_) => panic!(
                "Something went wrong with finding main! {:?}",
                syntax.lock().unwrap().compiling.iter().map(|pair| pair.data.name.clone()).collect::<Vec<_>>()
            ),
        };

        return Some(Arc::new(function.to_codeless()));
    }

    /// Compiles the main function
    pub fn compile(
        main: Arc<CodelessFinalizedFunction>,
        type_getter: &mut CompilerTypeGetter<'ctx>,
        functions: &Arc<DashMap<String, Arc<FinalizedFunction>>>,
        _structures: &Arc<DashMap<String, Arc<FinalizedStruct>>>,
    ) {
        instance_function(main, type_getter);

        let start = Instant::now();
        while !type_getter.compiling.borrow().is_empty() {
            if start.elapsed().as_secs() > 5 {
                panic!(
                    "Failed: {:?}",
                    type_getter.compiling.borrow().iter().map(|(_, func)| &func.data.name).collect::<Vec<_>>()
                )
            }

            let (function_type, function) = type_getter.compiling.borrow_mut().remove(0);

            if function.data.name.is_empty() {
                // The checker handles the poisoned functions
                continue;
            }

            let finalized_function;
            {
                finalized_function = if let Some(found) = functions.get(&function.data.name) {
                    found.clone()
                } else {
                    type_getter.compiling.borrow_mut().push((function_type, function));
                    continue;
                };
            }

            if finalized_function.code.expressions.len() == 0 {
                continue;
            }

            compile_block(
                &finalized_function.code,
                function_type,
                &mut type_getter.for_function(&finalized_function, function_type),
                &mut 0,
            );
        }

        //let pass_manager = PassManager::create(&self.compiler.module);

        /*unsafe {
            LLVMWriteBitcodeToFile(type_getter.compiler.module.as_mut_ptr(),
                                   CString::new(arguments.temp_folder.join("output.bc")
                                       .to_str().unwrap()).unwrap().as_ptr());
        }*/

        //print_formatted(type_getter.compiler.module.to_string());
    }
}
