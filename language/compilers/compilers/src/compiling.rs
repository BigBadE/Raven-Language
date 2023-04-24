use std::sync::{Arc, Mutex};
use syntax::ParsingError;
use syntax::syntax::Syntax;

pub trait Compiler {
    /// Compiles the main function and returns the main runner.
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>) -> Result<Option<i64>, Vec<ParsingError>>;
}