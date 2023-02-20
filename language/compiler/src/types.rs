use std::collections::HashMap;
use llvm_sys::prelude::{LLVMContextRef, LLVMTypeRef};

pub struct Types {
    pub types: HashMap<String, LLVMTypeRef>
}

impl Types {
    pub fn new(context: LLVMContextRef) -> Self {
        let types = HashMap::new();

        return Self {
            types
        };
    }
}