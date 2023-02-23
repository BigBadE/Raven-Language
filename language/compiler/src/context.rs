use alloc::ffi::CString;
use std::borrow::Cow;
use std::collections::HashMap;
use inkwell::module::Module;
use llvm_sys::core::{LLVMContextCreate, LLVMCreateBuilderInContext, LLVMModuleCreateWithNameInContext};
use llvm_sys::LLVMLinkage;
use llvm_sys::prelude::{LLVMBuilderRef, LLVMContextRef, LLVMModuleRef, LLVMTypeRef};
use crate::types::Type;

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

    pub fn add_function(name: &str, return_type: Box<dyn Type>, linkage: LLVMLinkage) -> Box<dyn Type> {

    }
}