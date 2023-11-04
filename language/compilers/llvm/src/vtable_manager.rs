use std::collections::HashMap;
use std::sync::Arc;
use inkwell::AddressSpace;
use inkwell::values::{BasicValue, GlobalValue};
use syntax::r#struct::StructData;
use syntax::types::FinalizedTypes;
use crate::type_getter::CompilerTypeGetter;


pub struct VTableManager<'ctx> {
    data: HashMap<(Arc<StructData>, Arc<StructData>), GlobalValue<'ctx>>,
}

impl<'ctx> VTableManager<'ctx> {
    pub fn new() -> Self {
        return VTableManager {
            data: HashMap::new()
        };
    }

    pub fn get_vtable(&mut self, type_getter: &mut CompilerTypeGetter<'ctx>, structure: &FinalizedTypes, target: &Arc<StructData>) -> GlobalValue<'ctx> {
        if let Some(found) = self.data.get(&(structure.inner_struct().data.clone(), target.clone())) {
            return found.clone();
        }
        let mut values = Vec::new();
        {
            let locked = type_getter.syntax.clone();
            let locked = locked.lock().unwrap();
            for found in locked.get_implementation_methods(structure, target).unwrap() {
                let func = type_getter.get_function(locked.functions.data.get(&found).unwrap());
                values.push(func.as_global_value().as_basic_value_enum());
            }
        }
        let structure = structure.inner_struct().data.clone();
        let value = type_getter.compiler.context.const_struct(values.as_slice(), false);
        let global = type_getter.compiler.module.add_global(value.get_type(),
                                                            Some(AddressSpace::default()), &format!("{}_vtable", structure.name));
        global.set_initializer(&value.as_basic_value_enum());
        self.data.insert((structure.clone(), target.clone()), global);
        return self.data.get(&(structure.clone(), target.clone())).unwrap().clone();
    }
}