use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use inkwell::module::Module;
use inkwell::OptimizationLevel;
use compilers::compiling::UnsafeFn;

pub struct CompilerImpl<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
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

    pub fn get_main<T, A>(&self) -> Option<UnsafeFn<T, A>> {
        let function = unsafe { self.execution_engine.get_function("main::main") };
        return match function {
            Ok(value) => Some(unsafe { value.into_raw() }),
            Err(_) => None
        };
    }
}