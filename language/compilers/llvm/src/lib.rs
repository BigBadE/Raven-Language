#![feature(get_mut_unchecked, box_into_inner)]
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use compilers::compiling::{Compiler, UnsafeFn};

use inkwell::context::Context;
use syntax::ParsingError;

use crate::compiler::CompilerImpl;
use crate::type_getter::CompilerTypeGetter;

pub mod internal;

pub mod compiler;
pub mod function_compiler;
pub mod type_getter;
pub mod type_waiter;
pub mod util;

pub struct LLVMCompiler {}

impl LLVMCompiler {
    pub fn new() -> Self {
        return Self {}
    }
}

impl<Args, Output> Compiler<Args, Output> for LLVMCompiler {
    fn compile(&self, syntax: &Arc<Mutex<syntax::syntax::Syntax>>) -> Result<Option<UnsafeFn<Args, Output>>, Vec<ParsingError>> {
        let context = Context::create();
        let result = CompilerTypeGetter::new(
            Rc::new(CompilerImpl::new(&context)), syntax.clone()).compile();
        let locked = syntax.lock().unwrap();

        return if locked.errors.is_empty() {
            result
        } else {
            Err(locked.errors.clone())
        }
    }
}