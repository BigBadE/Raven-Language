use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use crate::compiler::CompilerImpl;
use crate::function_compiler::{instance_function, instance_types};
use crate::internal::structs::get_internal_struct;
use crate::vtable_manager::VTableManager;
use inkwell::basic_block::BasicBlock;
use inkwell::execution_engine::JitFunction;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::AddressSpace;
use syntax::program::function::{CodelessFinalizedFunction, FinalizedFunction};
use syntax::program::syntax::{Main, Syntax};
use syntax::program::types::FinalizedTypes;
use syntax::VariableManager;

/// Data used when compiling a function
pub struct CompilerTypeGetter<'ctx> {
    /// Program syntax
    pub syntax: Arc<Mutex<Syntax>>,
    /// All generated VTables
    pub vtable: Rc<RefCell<VTableManager<'ctx>>>,
    /// The compiler
    pub compiler: Rc<CompilerImpl<'ctx>>,
    /// Functions being compiled
    pub compiling: Rc<RefCell<Vec<(FunctionValue<'ctx>, Arc<CodelessFinalizedFunction>)>>>,
    /// Current function's code blocks
    pub blocks: HashMap<String, BasicBlock<'ctx>>,
    /// The current block
    pub current_block: Option<BasicBlock<'ctx>>,
    /// Current function's variables
    pub variables: HashMap<String, (FinalizedTypes, BasicValueEnum<'ctx>)>,
}

impl<'ctx> CompilerTypeGetter<'ctx> {
    /// Creates a new CompilerTypeGetter
    pub fn new(compiler: Rc<CompilerImpl<'ctx>>, syntax: Arc<Mutex<Syntax>>) -> Self {
        return Self {
            syntax,
            vtable: Rc::new(RefCell::new(VTableManager::default())),
            compiler,
            compiling: Rc::new(RefCell::new(Vec::default())),
            blocks: HashMap::default(),
            current_block: None,
            variables: HashMap::default(),
        };
    }

    /// Adds the FinalizedFunction's fields to the CompilerTypeGetter
    pub fn for_function(&self, function: &Arc<FinalizedFunction>, llvm_function: FunctionValue<'ctx>) -> Self {
        let mut variables = self.variables.clone();
        let offset = function.fields.len() != llvm_function.count_params() as usize;
        for i in offset as usize..llvm_function.count_params() as usize {
            let field = &function.fields.get(i + offset as usize).unwrap().field;
            variables.insert(field.name.clone(), (field.field_type.clone(), llvm_function.get_nth_param(i as u32).unwrap()));
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

    /// Gets the LLVM version of the function
    pub fn get_function(&mut self, function: &Arc<CodelessFinalizedFunction>) -> FunctionValue<'ctx> {
        match self.compiler.module.get_function(&function.data.name) {
            Some(found) => found,
            None => {
                return instance_function(function.clone(), self);
            }
        }
    }

    /// Gets the LLVM version of the type
    pub fn get_type(&mut self, types: &FinalizedTypes) -> BasicTypeEnum<'ctx> {
        let found = match self.compiler.module.get_struct_type(&types.name()) {
            Some(found) => found.as_basic_type_enum(),
            None => get_internal_struct(self.compiler.context, &types.name()).unwrap_or_else(|| instance_types(types, self)),
        }
        .as_basic_type_enum();
        return match types {
            FinalizedTypes::Struct(_) | FinalizedTypes::Array(_) => found,
            FinalizedTypes::Reference(_) => found.ptr_type(AddressSpace::default()).as_basic_type_enum(),
            _ => panic!("Can't compile a generic! {:?}", found),
        };
    }

    /// Gets the target function that can be called directly from Rust
    pub(crate) fn get_target<T>(&self, target: &str) -> Option<JitFunction<'_, Main<T>>> {
        return unsafe {
            match self.compiler.execution_engine.get_function(target) {
                Ok(value) => Some(value),
                Err(_) => None,
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
