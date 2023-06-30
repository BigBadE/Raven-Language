#![feature(get_mut_unchecked, box_into_inner)]

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use inkwell::context::Context;
use syntax::function::Function;
use syntax::ParsingError;
use syntax::r#struct::Struct;
use syntax::syntax::{Compiler, Output, Syntax};

use crate::compiler::CompilerImpl;
use crate::type_getter::CompilerTypeGetter;

pub mod internal;
pub mod lifetimes;

pub mod compiler;
pub mod function_compiler;
pub mod type_getter;
pub mod util;

pub struct LLVMCompiler {
    compiling: Arc<HashMap<String, Arc<Function>>>,
    struct_compiling: Arc<HashMap<String, Arc<Struct>>>,
    context: Context
}

impl LLVMCompiler {
    pub fn new(compiling: Arc<HashMap<String, Arc<Function>>>, struct_compiling: Arc<HashMap<String, Arc<Struct>>>) -> Self {
        return Self {
            compiling,
            struct_compiling,
            context: Context::create()
        }
    }
}

impl Compiler for LLVMCompiler {
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>)
        -> Result<Option<Output>, Vec<ParsingError>> {
        let mut binding = CompilerTypeGetter::new(
            Rc::new(CompilerImpl::new(&self.context)), syntax.clone());

        let result = binding.compile(&self.compiling,
        &self.struct_compiling);

        let locked = syntax.lock().unwrap();

        return if locked.errors.is_empty() {
            match result {
                Ok(function) => match function {
                    Some(function) => Ok(Some(unsafe { function.call() })),
                    None => Ok(None)
                },
                Err(error) => Err(error)
            }
        } else {
            Err(locked.errors.clone())
        }
    }
}