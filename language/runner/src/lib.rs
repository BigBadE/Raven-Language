use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use compiler_llvm::LLVMCompiler;
use data::CompilerArguments;
use syntax::function::FinalizedFunction;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::Compiler;

pub mod runner;

pub fn get_compiler<T>(compiling: Arc<RwLock<HashMap<String, Arc<FinalizedFunction>>>>,
                       struct_compiling: Arc<RwLock<HashMap<String, Arc<FinalizedStruct>>>>,
                       arguments: CompilerArguments) -> Box<dyn Compiler<T> + Send + Sync> {
    return Box::new(match arguments.compiler.to_lowercase().as_str() {
        "llvm" => LLVMCompiler::new(compiling, struct_compiling, arguments),
        _ => panic!("Unknown compilers {}", arguments.compiler)
    });
}