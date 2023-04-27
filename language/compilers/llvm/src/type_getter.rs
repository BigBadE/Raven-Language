use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::execution_engine::JitFunction;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use syntax::function::Function;
use syntax::{ParsingError, VariableManager};
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::compiler::CompilerImpl;
use crate::function_compiler::{compile_block, instance_function, instance_struct};
use crate::internal::structs::get_internal_struct;
use crate::type_waiter::TypeWaiter;
use crate::util::print_formatted;

pub type Main = unsafe extern "C" fn() -> i64;

pub struct CompilerTypeGetter<'ctx> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub compiler: Rc<CompilerImpl<'ctx>>,
    pub compiling: Rc<Vec<(FunctionValue<'ctx>, Arc<Function>)>>,
    pub blocks: HashMap<String, BasicBlock<'ctx>>,
    pub current_block: Option<BasicBlock<'ctx>>,
    pub variables: HashMap<String, (Types, BasicValueEnum<'ctx>)>,
}

impl<'ctx> CompilerTypeGetter<'ctx> {
    pub fn new(compiler: Rc<CompilerImpl<'ctx>>, syntax: Arc<Mutex<Syntax>>) -> Self {
        return Self {
            compiler,
            syntax,
            compiling: Rc::new(Vec::new()),
            blocks: HashMap::new(),
            current_block: None,
            variables: HashMap::new(),
        };
    }

    pub fn for_function(&self, function: Function, llvm_function: FunctionValue<'ctx>) -> Self {
        let mut variables = self.variables.clone();
        let offset = function.fields.len() != llvm_function.count_params() as usize;
        for i in 0..llvm_function.count_params() as usize {
            let field = &function.fields.get(i + offset as usize).unwrap().field;
            variables.insert(field.name.clone(),
                             (field.field_type.clone(), llvm_function.get_nth_param(i as u32).unwrap()));
        }
        return Self {
            syntax: self.syntax.clone(),
            compiler: self.compiler.clone(),
            compiling: self.compiling.clone(),
            blocks: self.blocks.clone(),
            current_block: self.current_block.clone(),
            variables,
        };
    }

    pub fn get_function(&mut self, function: &Arc<Function>) -> FunctionValue<'ctx> {
        match self.compiler.module.get_function(&function.name) {
            Some(found) => found,
            None => {
                return instance_function(function.clone(), self);
            }
        }
    }

    pub fn get_type(&mut self, types: &Types) -> BasicTypeEnum<'ctx> {
        let found = match self.compiler.module.get_struct_type(&types.name()) {
            Some(found) => found.as_basic_type_enum(),
            None => get_internal_struct(self.compiler.context, &types.name()).unwrap_or(
                instance_struct(types.clone_struct(), self)
                    .as_basic_type_enum())
        }.as_basic_type_enum();
        return match types {
            Types::Struct(_) => found,
            Types::Reference(_) => found.ptr_type(AddressSpace::default()).as_basic_type_enum(),
            Types::GenericType(_, _) => panic!("Can't compile a generic!"),
            Types::Generic(_, _) => panic!("Can't compile a generic!")
        };
    }

    pub fn compile(&mut self) -> Result<Option<JitFunction<'_, Main>>, Vec<ParsingError>> {
        let found = self.syntax.lock().unwrap().functions.types.contains_key("main::main");

        if !found && self.syntax.lock().unwrap().async_manager.remaining != 0 {
            TypeWaiter::new(&mut self.syntax.lock().unwrap(), "main::main").wait();
        }

        let function = match self.syntax.lock().unwrap().functions.types.get("main::main") {
            Some(found) => found,
            None => return Ok(None)
        }.clone();

        instance_function(function, self);

        let mut errors = Vec::new();
        while !self.compiling.is_empty() {
            let (function_type, function) = unsafe {
                Rc::get_mut_unchecked(&mut self.compiling)
            }.remove(0);
            if !function.poisoned.is_empty() {
                for error in &function.poisoned {
                    errors.push(error.clone());
                }
                continue
            }
            compile_block(&function.code, function_type, self, &mut 0);
            self.compiler.builder.build_return(None);
        }

        print_formatted(self.compiler.module.to_string());
        return Ok(self.get_main());
    }

    fn get_main(&self) -> Option<JitFunction<'_, Main>> {
        return unsafe {
            match self.compiler.execution_engine.get_function("main::main") {
                Ok(value) => Some(value),
                Err(_) => None
            }
        };
    }
}

impl VariableManager for CompilerTypeGetter<'_> {
    fn get_variable(&self, name: &String) -> Option<Types> {
        return self.variables.get(name).map(|found| found.0.clone());
    }
}