use std::sync::{Arc, Mutex};
use syntax::{is_modifier, Modifier, ParsingError};
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use crate::output::TypesChecker;

pub async fn verify_struct(_process_manager: &TypesChecker, structure: &mut Struct,
                             _syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    if is_modifier(structure.modifiers, Modifier::Internal) {
        return Ok(());
    }

    return Ok(());
}