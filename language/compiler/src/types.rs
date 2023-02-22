use std::collections::HashMap;
use inkwell::values::BasicValue;
use ast::code::{Effects, MathOperator};
use crate::compiler::Compiler;

pub struct TypeManager {
    pub types: HashMap<String, Box<dyn Type>>
}

impl TypeManager {
    pub fn new() -> Self {
        return Self {
            types: HashMap::new()
        }
    }

    pub fn get_type(&self, name: &str) -> Option<&Box<dyn Type>> {
        return self.types.get(name);
    }
}

pub trait Type {
    fn math_operation(operation: MathOperator, compiler: &Compiler, target: Effects);

}

pub struct Value<'ctx> {
    value_type: &'ctx dyn Type,
    value: Box<dyn BasicValue<'ctx>>,
}

impl<'ctx> Value<'ctx> {
    pub fn new(value_type: &'ctx dyn Type, value: dyn BasicValue<'ctx>) -> Self {
        return Self {
            value_type,
            value: Box::new(value)
        }
    }

    pub fn value(&self) -> &dyn BasicValue<'ctx> {
        return self.value.as_ref();
    }
}