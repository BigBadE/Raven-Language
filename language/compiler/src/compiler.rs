use std::collections::HashMap;
use inkwell::values::FunctionValue;
use llvm_sys::prelude::LLVMTypeRef;
use crate::context::Context;
use crate::execution::Executor;
use crate::file::FileStructureImpl;
use crate::function_compiler::compile_function;
use crate::types::Type;

type Main = unsafe extern "C" fn();

pub struct Compiler {
    pub context: Context,
    executor: Executor,
}

impl Compiler {
    pub fn new() -> Self {
        let context = Context::new("main");
        return Self {
            executor: Executor::new(&context),
            context,
        };
    }

    pub fn get_type(&self, name: &String) -> &Box<dyn Type> {
        return self.context.types.get(name).expect(&*("Couldn't find type named ".to_string() + name));
    }

    pub fn compile(&mut self, content: FileStructureImpl) -> Option<Main> {
        let program = parser::parse(Box::new(content));

        match program.main {
            Some(main) => {
                let function = program.static_functions.get(main.as_str()).unwrap();
                self.functions.insert(function.name.value.clone(), compile_function(function, &self));
                return Some(self.executor.get_function("main::main"));
            },
            None => {}
        }

        return None;
    }
}