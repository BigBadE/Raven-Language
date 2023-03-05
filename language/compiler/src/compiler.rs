use std::ops::Deref;
use std::rc::Rc;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::Module;
use inkwell::OptimizationLevel;
use inkwell::types::BasicTypeEnum;
use ast::type_resolver::TypeResolver;
use ast::types::Types;
use crate::function_compiler::{compile_function, get_function_value};
use crate::types::type_resolver::CompilerTypeResolver;

type Main = unsafe extern "C" fn() -> i64;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
    pub type_manager: CompilerTypeResolver<'ctx>,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(type_manager: CompilerTypeResolver<'ctx>, context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();
        return Self {
            context,
            module,
            builder: context.create_builder(),
            execution_engine,
            type_manager,
        };
    }

    pub fn get_type(&self, name: &String) -> Rc<Types> {
        return self.type_manager.get_type(name).expect(&*("Couldn't find type named ".to_string() + name)).clone();
    }

    pub fn get_llvm_type(&self, types: &Types) -> &BasicTypeEnum {
        for (name, found_types) in self.type_manager.types.deref() {
            if found_types.deref() == types {
                return self.type_manager.llvm_types.get(name).unwrap();
            }
        }
        panic!("Couldn't find type?");
    }

    pub fn compile(&mut self) -> Option<JitFunction<'ctx, Main>> {

        //Add them to functions for function calls
        for (_name, (function, function_value)) in
        unsafe { Rc::get_mut_unchecked(&mut self.type_manager.functions.clone()) } {
            *function_value = Some(get_function_value(function, self));
        }

        //Compile
        for (function, _function_value) in self.type_manager.functions.values() {
            compile_function(function, self);
        }

        match self.type_manager.functions.get("main::main") {
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