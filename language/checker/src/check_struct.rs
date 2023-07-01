use std::sync::{Arc, Mutex};
use syntax::{is_modifier, Modifier, ParsingError};
use syntax::r#struct::StructData;
use syntax::syntax::Syntax;
use crate::output::TypesChecker;

pub async fn verify_struct(_process_manager: &TypesChecker, structure: &mut StructData,
                           _syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    if is_modifier(structure.modifiers, Modifier::Internal) {
        return Ok(());
    }

    return Ok(());
}