use std::collections::HashMap;
use inkwell::values::{BasicValue, BasicValueEnum, FloatValue};
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
    fn math_operation<'ctx>(&self, operation: MathOperator, compiler: &Compiler, current: Value, target: Value) -> Value<'ctx>;
}

pub struct Value<'ctx> {
    pub value_type: &'ctx dyn Type,
    pub value: Box<dyn BasicValue<'ctx>>,
}

impl<'ctx> Value<'ctx> {
    pub fn new(value_type: &'ctx dyn Type, value: Box<dyn BasicValue<'ctx>>) -> Self {
        return Self {
            value_type,
            value
        }
    }
}

impl<'ctx> Clone for Value<'ctx> {
    fn clone(&self) -> Self {
        return Value {
            value_type: self.value_type,
            value: Box::new(Value::new(self.value.as_value_ref()))
        }
    }
}