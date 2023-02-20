use llvm_sys::core::{LLVMContextCreate, LLVMCreateBuilderInContext};
use llvm_sys::prelude::{LLVMBuilderRef, LLVMContextRef};
use ast::TopElement;

pub struct Compiler {
    context: LLVMContextRef,
    builder: LLVMBuilderRef
}

impl Compiler {
    pub fn new() -> Self {
        unsafe {
            let context = LLVMContextCreate();
            return Self {
                context,
                builder: LLVMCreateBuilderInContext(context)
            }
        }
    }

    pub fn compile(&self, content: String) -> Box<[u8]> {
        for element in parser::parse(content) {
            match element {
                TopElement::Struct(class_type) => {},
                TopElement::Function(function) => {

                }
            }
        }

        return Box::new([0]);
    }
}

macro_rules! c_str {
    ($s:expr) => (
        concat!($s, "\0").as_ptr() as *const i8
    );
}