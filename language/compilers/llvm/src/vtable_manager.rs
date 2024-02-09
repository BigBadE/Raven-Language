use crate::type_getter::CompilerTypeGetter;
use inkwell::values::{BasicValue, GlobalValue};
use inkwell::AddressSpace;
use std::collections::HashMap;
use std::sync::Arc;
use syntax::function::CodelessFinalizedFunction;
use syntax::r#struct::StructData;
use syntax::types::FinalizedTypes;

/// A struct to manage Virtual Tables
#[derive(Default)]
pub struct VTableManager<'ctx> {
    // All the current generated VTables sorted by the parent type and the implemented trait
    data: HashMap<(Arc<StructData>, Arc<StructData>), GlobalValue<'ctx>>,
}

impl<'ctx> VTableManager<'ctx> {
    /// Gets a vtable for the given program and target trait, generating one if it doesn't exist
    pub fn get_vtable(
        &mut self,
        type_getter: &mut CompilerTypeGetter<'ctx>,
        target: &FinalizedTypes,
        structure: &FinalizedTypes,
        functions: &Vec<Arc<CodelessFinalizedFunction>>,
    ) -> GlobalValue<'ctx> {
        if let Some(found) = self.data.get(&(structure.inner_struct().data.clone(), target.inner_struct().data.clone())) {
            return *found;
        }
        let mut values = Vec::default();
        {
            for found in functions {
                let func = type_getter.get_function(found);
                values.push(func.as_global_value().as_basic_value_enum());
            }
        }
        let structure = structure.inner_struct().data.clone();
        let value = type_getter.compiler.context.const_struct(values.as_slice(), false);
        let global = type_getter.compiler.module.add_global(
            value.get_type(),
            Some(AddressSpace::default()),
            &format!("{}_vtable", structure.name),
        );
        global.set_initializer(&value.as_basic_value_enum());
        self.data.insert((structure.clone(), target.inner_struct().data.clone()), global);
        return *self.data.get(&(structure.clone(), target.inner_struct().data.clone())).unwrap();
    }
}
