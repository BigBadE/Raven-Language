use crate::type_getter::CompilerTypeGetter;
use inkwell::values::{BasicValue, GlobalValue};
use inkwell::AddressSpace;
use std::collections::HashMap;
use std::sync::Arc;
use syntax::r#struct::StructData;
use syntax::types::FinalizedTypes;

#[derive(Default)]
pub struct VTableManager<'ctx> {
    data: HashMap<(Arc<StructData>, Arc<StructData>), GlobalValue<'ctx>>,
}

impl<'ctx> VTableManager<'ctx> {
    pub fn new() -> Self {
        return VTableManager {
            data: HashMap::default(),
        };
    }

    pub fn get_vtable(
        &mut self,
        type_getter: &mut CompilerTypeGetter<'ctx>,
        structure: &FinalizedTypes,
        target: &FinalizedTypes,
    ) -> GlobalValue<'ctx> {
        if let Some(found) = self.data.get(&(
            structure.inner_struct().data.clone(),
            target.inner_struct().data.clone(),
        )) {
            return *found;
        }
        let mut values = Vec::default();
        {
            let locked = type_getter.syntax.clone();
            let locked = locked.lock().unwrap();

            for found in locked
                .get_implementation_methods(structure, &target.unflatten())
                .unwrap()
            {
                let func = type_getter.get_function(locked.functions.data.get(&found).unwrap());
                values.push(func.as_global_value().as_basic_value_enum());
            }
        }
        let structure = structure.inner_struct().data.clone();
        let value = type_getter
            .compiler
            .context
            .const_struct(values.as_slice(), false);
        let global = type_getter.compiler.module.add_global(
            value.get_type(),
            Some(AddressSpace::default()),
            &format!("{}_vtable", structure.name),
        );
        global.set_initializer(&value.as_basic_value_enum());
        self.data.insert(
            (structure.clone(), target.inner_struct().data.clone()),
            global,
        );
        return *self
            .data
            .get(&(structure.clone(), target.inner_struct().data.clone()))
            .unwrap();
    }
}
