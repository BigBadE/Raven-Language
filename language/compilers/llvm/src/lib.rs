#![feature(get_mut_unchecked, box_into_inner)]

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc; use no_deadlocks::Mutex;

use inkwell::context::Context;
use syntax::function::FinalizedFunction;
use syntax::ParsingError;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::{Compiler, Syntax, Main};

use crate::compiler::CompilerImpl;
use crate::type_getter::CompilerTypeGetter;

pub mod internal;
pub mod lifetimes;

pub mod compiler;
pub mod function_compiler;
pub mod type_getter;
pub mod util;

pub struct LLVMCompiler {
    compiling: Arc<HashMap<String, Arc<FinalizedFunction>>>,
    struct_compiling: Arc<HashMap<String, Arc<FinalizedStruct>>>,
    context: Context
}

impl LLVMCompiler {
    pub fn new(compiling: Arc<HashMap<String, Arc<FinalizedFunction>>>,
               struct_compiling: Arc<HashMap<String, Arc<FinalizedStruct>>>) -> Self {
        return Self {
            compiling,
            struct_compiling,
            context: Context::create()
        }
    }
}

impl<T> Compiler<T> for LLVMCompiler {
    fn compile(&self, target: &str, syntax: &Arc<Mutex<Syntax>>)
        -> Result<Option<Main<T>>, Vec<ParsingError>> {
        let mut binding = CompilerTypeGetter::new(
            Rc::new(CompilerImpl::new(&self.context)), syntax.clone());

        let result = binding.compile(target, &self.compiling,
        &self.struct_compiling);

        let locked = syntax.lock().unwrap();

        return if locked.errors.is_empty() {
            match result {
                Ok(function) => match function {
                    Some(function) => Ok(Some(unsafe { function.as_raw() })),
                    None => Ok(None)
                },
                Err(error) => Err(error)
            }
        } else {
            Err(locked.errors.clone())
        }
    }
}