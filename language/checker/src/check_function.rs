use std::sync::{Arc, Mutex};
use syntax::function::Function;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::check_code::verify_code;

pub async fn verify_function(mut function: Arc<Function>, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    verify_code(&mut unsafe { Arc::get_mut_unchecked(&mut function) }.code, syntax).await?;
    return Ok(());
}