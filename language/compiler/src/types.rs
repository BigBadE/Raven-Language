use llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef};
use ast::code::MathOperator;
use crate::compiler::Compiler;

pub trait Type {
    fn math_operation(&self, operation: MathOperator, compiler: &Compiler, current: Value, target: Value) -> Option<Value>;

    fn get_type(&self) -> LLVMTypeRef;
    
    fn clone(&self) -> Box<dyn Type>;
}

pub struct FunctionType {

}

impl FunctionType for Type {

}

pub struct Value {
    pub value_type: Box<dyn Type>,
    pub value: LLVMValueRef,
}

impl Value {
    pub fn new(value_type: Box<dyn Type>, value: LLVMValueRef) -> Self {
        return Self {
            value_type,
            value
        }
    }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        return Value {
            value_type: self.value_type.clone(),
            value: self.value.clone()
        }
    }
}