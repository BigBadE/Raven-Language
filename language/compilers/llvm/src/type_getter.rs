use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;

use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::execution_engine::JitFunction;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use checker::EmptyNameResolver;
use syntax::function::{CodelessFinalizedFunction, FinalizedFunction};
use syntax::{ParsingError, VariableManager};
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::{Main, Syntax};
use syntax::types::FinalizedTypes;
use crate::compiler::CompilerImpl;
use crate::function_compiler::{compile_block, instance_function, instance_types};
use crate::internal::structs::get_internal_struct;
use crate::main_future::MainFuture;
use crate::vtable_manager::VTableManager;

pub struct CompilerTypeGetter<'ctx> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub vtable: Rc<VTableManager<'ctx>>,
    pub compiler: Rc<CompilerImpl<'ctx>>,
    pub compiling: Rc<Vec<(FunctionValue<'ctx>, Arc<CodelessFinalizedFunction>)>>,
    pub blocks: HashMap<String, BasicBlock<'ctx>>,
    pub current_block: Option<BasicBlock<'ctx>>,
    pub variables: HashMap<String, (FinalizedTypes, BasicValueEnum<'ctx>)>,
}

unsafe impl Sync for CompilerTypeGetter<'_> {

}

unsafe impl Send for CompilerTypeGetter<'_> {

}

impl<'ctx> CompilerTypeGetter<'ctx> {
    pub fn new(compiler: Rc<CompilerImpl<'ctx>>, syntax: Arc<Mutex<Syntax>>) -> Self {
        return Self {
            syntax,
            vtable: Rc::new(VTableManager::new()),
            compiler,
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
            vtable: self.vtable.clone(),
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
                    instance_types(types, self))
        }.as_basic_type_enum();
        return match types {
            FinalizedTypes::Struct(_) | FinalizedTypes::Array(_) => found,
            FinalizedTypes::Reference(_) => found.ptr_type(AddressSpace::default()).as_basic_type_enum(),
            _ => panic!("Can't compile a generic! {:?}", found)
        };
    }

    pub async fn compile<T>(&mut self, target: String, syntax: &Arc<Mutex<Syntax>>, functions: &Arc<RwLock<HashMap<String, Arc<FinalizedFunction>>>>,
                      _structures: &Arc<RwLock<HashMap<String, Arc<FinalizedStruct>>>>)
                      -> Option<JitFunction<'_, Main<T>>> {
        match Syntax::get_function(syntax.clone(), ParsingError::empty(), target.clone(),
                             Box::new(EmptyNameResolver {}), false).await {
            Ok(_) => {},
            Err(_) => return None
        };

        let target = target.as_str();

        let function = MainFuture { syntax: syntax.clone() }.await;

        instance_function(Arc::new(function.to_codeless()), self);

        while !self.compiling.is_empty() {
            let (function_type, function) = unsafe {
                Rc::get_mut_unchecked(&mut self.compiling)
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
                        Rc::get_mut_unchecked(&mut self.compiling)
                    }.push((function_type, function));
                    continue
                };
            }
            compile_block(&finalized_function.code, function_type,
                          &mut self.for_function(&finalized_function, function_type), &mut 0);
        }

        //let pass_manager = PassManager::create(&self.compiler.module);

        //unsafe {
            //LLVMWriteBitcodeToFile(self.compiler.module.as_mut_ptr(), c_str("main.bc"));
        //}

        //print_formatted(self.compiler.module.to_string());
        return self.get_target(target);
    }

    fn get_target<T>(&self, target: &str) -> Option<JitFunction<'_, Main<T>>> {
        return unsafe {
            match self.compiler.execution_engine.get_function(target) {
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