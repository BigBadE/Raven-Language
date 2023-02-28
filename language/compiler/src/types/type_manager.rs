use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::types::{BasicType, BasicTypeEnum};

pub struct TypeManager<'ctx> {
    pub types: HashMap<String, BasicTypeEnum<'ctx>>
}

impl<'ctx> TypeManager<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let mut types = HashMap::new();
        types.insert("f64".to_string(), context.f64_type().as_basic_type_enum());
        types.insert("i64".to_string(), context.i64_type().as_basic_type_enum());
        return Self {
            types
        }
    }

    pub fn get_type(&self, name: &str) -> Option<&BasicTypeEnum> {
        return self.types.get(name);
    }

    pub fn get_type_err(&self, name: &str) -> &BasicTypeEnum {
        return self.types.get(name).expect(format!("Unknown type {}", name).as_str());
    }
}