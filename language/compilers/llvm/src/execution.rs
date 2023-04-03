use alloc::borrow::Cow;
use std::ffi::{CStr, CString};
use std::ptr;
use std::mem::MaybeUninit;
use llvm_sys::execution_engine::{LLVMCreateJITCompilerForModule, LLVMExecutionEngineRef, LLVMGetExecutionEngineTargetData, LLVMGetFunctionAddress};
use llvm_sys::target::LLVMTargetDataRef;
use crate::context::Context;

pub struct Executor {
    engine: LLVMExecutionEngineRef,
    target_data: LLVMTargetDataRef
}

impl Executor {
    pub fn new(context: &Context) -> Self {
        let mut engine = MaybeUninit::unint();
        let mut error = MaybeUninit::uninit();

        unsafe {
            let failed = LLVMCreateJITCompilerForModule(engine.as_mut_ptr(), context.module, 2, error.as_mut_ptr());

            if failed == 1 {
                panic!("{:?}", unsafe { CStr::from_ptr(error.assume_init()) });
            }
        }


        return Self {
            engine: engine.assume_init(),
            target_data: unsafe { LLVMGetExecutionEngineTargetData(engine) }
        }
    }

    pub fn get_function<F>(&self, name: &str) -> F {
        let address = unsafe { LLVMGetFunctionAddress(self.engine, Cow::new(CString::new(name)).as_ptr()) };

        if address == 0 {
            panic!("Function {} not found", name);
        }

        return unsafe { ptr::read(address as *const u8 as *const F) };
    }
}
