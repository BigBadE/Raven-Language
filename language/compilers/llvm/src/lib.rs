#![feature(get_mut_unchecked, box_into_inner)]
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use compilers::compiling::Compiler;

use inkwell::context::Context;
use syntax::ParsingError;
use syntax::syntax::Syntax;

use crate::compiler::CompilerImpl;
use crate::type_getter::CompilerTypeGetter;

pub mod internal;

pub mod compiler;
pub mod function_compiler;
pub mod type_getter;
pub mod type_waiter;
pub mod util;

pub struct LLVMCompiler {
    context: Context
}

impl LLVMCompiler {
    pub fn new() -> Self {
        return Self {
            context: Context::create()
        }
    }
}

impl Compiler for LLVMCompiler {
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>)
        -> Result<Option<i64>, Vec<ParsingError>> {
        let mut binding = CompilerTypeGetter::new(
            Rc::new(CompilerImpl::new(&self.context)), syntax.clone());
        let result = binding.compile();
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