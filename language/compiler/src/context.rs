use alloc::ffi::CString;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Deref;
use llvm_sys::core::{LLVMAddFunction, LLVMContextCreate, LLVMCreateBuilderInContext, LLVMModuleCreateWithNameInContext, LLVMSetLinkage};
use llvm_sys::LLVMLinkage;
use llvm_sys::prelude::{LLVMBuilderRef, LLVMContextRef, LLVMModuleRef, LLVMTypeRef};
use ast::code::Field;
use crate::types::{FunctionType, Type};

pub struct Context {
    pub context: LLVMContextRef,
    pub module: LLVMModuleRef,
    pub builder: LLVMBuilderRef,
    pub functions: HashMap<String, Box<dyn Type>>,
    pub types: HashMap<String, Box<dyn Type>>
}

impl Context {
    pub fn new(module_name: &str) -> Self {
        unsafe {
            let context = LLVMContextCreate();
            return Self {
                module: LLVMModuleCreateWithNameInContext(Cow::from(CString::new(module_name).unwrap()).as_ptr(), context),
                builder: LLVMCreateBuilderInContext(context),
                types: Self::init_types(context),
                functions: HashMap::new(),
                context
            }
        }
    }

    pub fn init_types(context: LLVMContextRef) -> HashMap<String, Box<dyn Type>> {
        let types = HashMap::new();
        todo!();
        return types;
    }

    pub fn add_function(&self, name: &str, return_type: &Box<dyn Type>, params: &Vec<Field>, linkage: Option<LLVMLinkage>) -> FunctionType {
        unsafe {
            let function =
                LLVMAddFunction(self.module, Cow::from(CString::new(name).unwrap()).as_ptr(), return_type.get_type());

            if linkage.is_some() {
                LLVMSetLinkage(function, linkage.unwrap());
            }

            return function;
        }
    }
}