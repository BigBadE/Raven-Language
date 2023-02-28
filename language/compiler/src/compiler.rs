use std::collections::HashMap;
use std::task::Waker;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::Module;
use inkwell::OptimizationLevel;
use inkwell::types::BasicTypeEnum;
use inkwell::values::FunctionValue;
use crate::file::FileStructureImpl;
use crate::function_compiler::{compile_function, get_function_value};
use crate::types::type_manager::TypeManager;

type Main = unsafe extern "C" fn() -> i64;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
    pub functions: HashMap<String, (Option<String>, FunctionValue<'ctx>)>,
    pub types: &'ctx TypeManager<'ctx>,
    pub waiting: HashMap<String, Waker>
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, types: &'ctx TypeManager<'ctx>) -> Self {
        let module = context.create_module("main");
        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();
        return Self {
            context,
            module,
            builder: context.create_builder(),
            execution_engine,
            functions: HashMap::new(),
            types,
            waiting: HashMap::new()
        };
    }

    pub fn get_type(&self, name: &String) -> &BasicTypeEnum {
        return self.types.types.get(name).expect(&*("Couldn't find type named ".to_string() + name));
    }

    pub fn compile(&mut self, content: FileStructureImpl) -> Option<JitFunction<Main>> {
        let program = parser::parse(Box::new(content));

        //Add them to functions for type resolving
        for (name, function) in &program.static_functions {
            self.functions.insert(name.clone(), (function.return_type.clone(), get_function_value(function, &self)));
        }

        //Compile
        for (_name, function) in &program.static_functions {
            compile_function(function, &self);
        }

        match program.main {
            Some(_main) => {
                let function =  unsafe { self.execution_engine.get_function("main::main") };
                return match function {
                    Ok(value) => Some(value),
                    Err(error) => panic!("{}", error)
                }
            },
            None => {}
        }

        return None;
    }
}