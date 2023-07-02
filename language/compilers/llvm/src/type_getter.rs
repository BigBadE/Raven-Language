use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::execution_engine::JitFunction;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use syntax::function::{CodelessFinalizedFunction, FinalizedFunction};
use syntax::{ParsingError, VariableManager};
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::{Output, Syntax};
use syntax::types::FinalizedTypes;
use crate::compiler::CompilerImpl;
use crate::function_compiler::{compile_block, instance_function, instance_struct};
use crate::internal::structs::get_internal_struct;
use crate::util::print_formatted;

pub type Main = unsafe extern "C" fn() -> Output;

pub struct CompilerTypeGetter<'ctx> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub compiler: Rc<CompilerImpl<'ctx>>,
    pub compiling: Rc<Vec<(FunctionValue<'ctx>, Arc<CodelessFinalizedFunction>)>>,
    pub blocks: HashMap<String, BasicBlock<'ctx>>,
    pub current_block: Option<BasicBlock<'ctx>>,
    pub variables: HashMap<String, (FinalizedTypes, BasicValueEnum<'ctx>)>,
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

    pub fn for_function(&self, function: &Arc<FinalizedFunction>, llvm_function: FunctionValue<'ctx>) -> Self {
        let mut variables = self.variables.clone();
        let offset = function.fields.len() != llvm_function.count_params() as usize;
        for i in offset as usize..llvm_function.count_params() as usize {
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

    pub fn get_function(&mut self, function: &Arc<CodelessFinalizedFunction>) -> FunctionValue<'ctx> {
        match self.compiler.module.get_function(&function.data.name) {
            Some(found) => found,
            None => {
                return instance_function(function.clone(), self);
            }
        }
    }

    pub fn get_type(&mut self, types: &FinalizedTypes) -> BasicTypeEnum<'ctx> {
        let found = match self.compiler.module.get_struct_type(&types.name()) {
            Some(found) => found.as_basic_type_enum(),
            None => get_internal_struct(self.compiler.context, &types.name()).unwrap_or(
                instance_struct(types.inner_struct(), self)
                    .as_basic_type_enum())
        }.as_basic_type_enum();
        return match types {
            FinalizedTypes::Struct(_) => found,
            FinalizedTypes::Reference(_) => found.ptr_type(AddressSpace::default()).as_basic_type_enum(),
            _ => panic!("Can't compile a generic!")
        };
    }

    pub fn compile(&mut self, functions: &HashMap<String, Arc<FinalizedFunction>>, _structures: &HashMap<String, Arc<FinalizedStruct>>)
                   -> Result<Option<JitFunction<'_, Main>>, Vec<ParsingError>> {
        while !functions.contains_key("main::main") {
            //Waiting
            thread::yield_now();
        }

        let function = match functions.get("main::main") {
            Some(found) => found,
            None => return Ok(None)
        }.clone();

        instance_function(Arc::new(function.to_codeless()), self);

        let mut errors = Vec::new();
        while !self.compiling.is_empty() {
            let (function_type, function) = unsafe {
                Rc::get_mut_unchecked(&mut self.compiling)
            }.remove(0);
            let function = if let Some(found) = functions.get(&function.data.name) {
                found
            } else {
                unsafe {
                    Rc::get_mut_unchecked(&mut self.compiling)
                }.push((function_type, function));
                continue
            };
            if !function.data.poisoned.is_empty() {
                for error in &function.data.poisoned {
                    errors.push(error.clone());
                }
                continue
            }
            compile_block(&function.code, function_type,
                          &mut self.for_function(&function, function_type), &mut 0);
            print_formatted(function_type.to_string());
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

impl Debug for CompilerTypeGetter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.variables.fmt(f);
    }
}

impl VariableManager for CompilerTypeGetter<'_> {
    fn get_variable(&self, name: &String) -> Option<FinalizedTypes> {
        return self.variables.get(name).map(|found| found.0.clone());
    }
}