use std::sync::{Arc, Mutex};
use syntax::ParsingError;
use syntax::syntax::Syntax;

pub trait Compiler<Args, Output>: Send + Sync {
    /// Compiles the main function and returns the main runner.
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>) -> Result<Option<UnsafeFn<Args, Output>>, Vec<ParsingError>>;
}

pub type UnsafeFn<Args, Output> = unsafe extern "C" fn(Args) -> Output;