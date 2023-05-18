use std::sync::{Arc, Mutex};
use syntax::ParsingError;
use syntax::syntax::Syntax;

pub type Output = bool;

pub trait Compiler {
    /// Compiles the main function and returns the main runner.
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>) -> Result<Option<Output>, Vec<ParsingError>>;
}