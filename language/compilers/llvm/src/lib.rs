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
pub mod execution;
pub mod function_compiler;
pub mod type_getter;
pub mod util;

pub struct LLVMCompiler {}

impl LLVMCompiler {
    pub fn new() -> Self {
        return Self {}
    }
}

impl<Args, Output> Compiler<Args, Output> for LLVMCompiler {
    fn compile(&self, syntax: &Arc<Mutex<syntax::syntax::Syntax>>) -> Result<UnsafeFn<Args, Output>, Vec<ParsingError>> {
        let context = Context::create();
        return CompilerTypeGetter::new(Rc::new(CompilerImpl::new(&context)), syntax.clone()).compile();
    }
}