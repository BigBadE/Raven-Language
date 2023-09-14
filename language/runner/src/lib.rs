use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;
use compiler_llvm::LLVMCompiler;
use syntax::function::FinalizedFunction;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::Compiler;

pub mod runner;

pub fn get_compiler<T>(compiling: Arc<HashMap<String, Arc<FinalizedFunction>>>, struct_compiling: Arc<HashMap<String, Arc<FinalizedStruct>>>,
                       name: String) -> Box<dyn Compiler<T>> {
    return Box::new(match name.to_lowercase().as_str() {
        "llvm" => LLVMCompiler::new(compiling, struct_compiling),
        _ => panic!("Unknown compilers {}", name)
    });
}