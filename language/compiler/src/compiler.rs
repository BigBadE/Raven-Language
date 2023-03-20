use std::borrow::BorrowMut;
use std::mem;
use std::ops::Deref;
use std::rc::Rc;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::Module;
use inkwell::OptimizationLevel;
use inkwell::types::BasicType;
use ast::code::{Field, MemberField};
use ast::type_resolver::FinalizedTypeResolver;
use ast::types::ResolvableTypes::Resolved;
use crate::function_compiler::compile_function;
use crate::types::type_resolver::ParserTypeResolver;

type Main = unsafe extern "C" fn() -> i64;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
    pub type_manager: ParserTypeResolver,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(type_manager: ParserTypeResolver, context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();

        return Self {
            type_manager,
            module,
            context,
            builder: context.create_builder(),
            execution_engine,
        };
    }

    pub fn compile(mut self) -> Option<JitFunction<'ctx, Main>> {
        let mut temp = ParserTypeResolver::new();
        mem::swap(&mut temp, &mut self.type_manager);
        let type_manager = temp.finalize(self.context, &self.module);
        print!("{}", type_manager);

        //Compile
        for (function, _function_value) in type_manager.functions.values() {
            if !function.generics.is_empty() {
                continue
            }

            let type_manager = type_manager.for_func(&function.name);
            compile_function(function, &self, &type_manager);
        }

        match type_manager.functions.get("main::main") {
            Some(_main) => {
                let function = unsafe { self.execution_engine.get_function("main::main") };
                return match function {
                    Ok(value) => Some(value),
                    Err(error) => panic!("{}", error)
                };
            }
            None => {}
        }

        return None;
    }
}