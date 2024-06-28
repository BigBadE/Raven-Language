use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::Arc;
use syntax::async_util::AsyncDataGetter;
use syntax::program::r#struct::{FinalizedStruct, StructData};

use crate::compiler::CompilerImpl;
use crate::function_compiler::{instance_function, instance_types};
use crate::internal::structs::get_internal_struct;
use crate::vtable_manager::VTableManager;
use inkwell::basic_block::BasicBlock;
use inkwell::execution_engine::JitFunction;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::AddressSpace;
use syntax::program::function::{display_parenless, CodelessFinalizedFunction, FinalizedFunction, FunctionData};
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
        let mut types = types.clone();
        self.fix_generic_struct(&mut types);
        let found = match self.compiler.module.get_struct_type(&types.name()) {
            Some(found) => found.as_basic_type_enum(),
            None => {
                get_internal_struct(self.compiler.context, &types.name()).unwrap_or_else(|| instance_types(&types, self))
            }
        }
        .as_basic_type_enum();
        return match types {
            FinalizedTypes::Struct(_) | FinalizedTypes::Array(_) => found,
            FinalizedTypes::Reference(_) => self.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
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

    pub fn fix_generic_struct(&self, types: &mut FinalizedTypes) {
        match types {
            FinalizedTypes::Reference(inner) | FinalizedTypes::Array(inner) => self.fix_generic_struct(inner),
            FinalizedTypes::GenericType(base, bounds) => {
                let base = base.inner_struct();
                if bounds.is_empty() {
                    *types = FinalizedTypes::Struct(base.clone());
                    // If there are no bounds, we're good.
                    return;
                }
                let name = format!("{}<{}>", base.data.name, display_parenless(&bounds, ", "));
                // If this type has already been flattened with these args, return that.
                if self.syntax.lock().structures.types.contains_key(&name) {
                    let base;
                    {
                        let locked = self.syntax.lock();
                        // skipcq: RS-W1070 Initialization of a value can't use clone_from
                        let data = locked.structures.types.get(&name).unwrap().clone();
                        base = locked.structures.data.get(&data).unwrap().clone();
                    }
                    *types = FinalizedTypes::Struct(base);
                } else {
                    // Clone the type and add the new type to the structures.
                    let mut other = StructData::clone(&base.data);
                    other.name.clone_from(&name);

                    // Update the program's functions
                    for function in &mut other.functions {
                        let mut temp = FunctionData::clone(function);
                        temp.name = format!("{}::{}", name, temp.name.split("::").last().unwrap());
                        let temp = Arc::new(temp);
                        *function = temp;
                    }

                    let arc_other = Arc::new(other);

                    // Get the FinalizedStruct and degeneric it.
                    let mut data = FinalizedStruct::clone(self.syntax.lock().structures.data.get(&base.data).unwrap());
                    data.data.clone_from(&arc_other);

                    let mut struct_generics = HashMap::new();
                    let mut i = 0;
                    for (name, _types) in &base.generics {
                        struct_generics.insert(name.clone(), bounds[i].clone());
                        i += 1;
                    }

                    // Update the program's fields
                    for field in &mut data.fields {
                        simple_degeneric(&mut field.field.field_type, &struct_generics);
                    }

                    let data = Arc::new(data);
                    // Add the flattened type to the syntax
                    {
                        let mut locked = self.syntax.lock();
                        locked.structures.add_data(arc_other, data.clone());
                    }
                    *types = FinalizedTypes::Struct(data.clone());
                }
            }
            _ => {}
        }
    }
}

pub fn simple_degeneric(degenericing: &mut FinalizedTypes, generics: &HashMap<String, FinalizedTypes>) {
    match degenericing {
        FinalizedTypes::Generic(name, _bounds) => {
            *degenericing = generics.get(name).unwrap().clone();
        }
        FinalizedTypes::Struct(_) => {}
        FinalizedTypes::Array(inner) | FinalizedTypes::Reference(inner) => {
            simple_degeneric(inner, generics);
        }
        _ => unreachable!(),
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
