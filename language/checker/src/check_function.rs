use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex};
use syntax::function::{CodeStatus, Function};
use syntax::{is_modifier, Modifier, ParsingError, VariableManager};
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_code::verify_code;
use crate::output::TypesChecker;

pub async fn verify_function(process_manager: &TypesChecker, function: &mut Function, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    if is_modifier(function.modifiers, Modifier::Internal) {
        return Ok(());
    }

    let mut variable_manager = CheckerVariableManager { variables: HashMap::new() };

    for argument in &function.fields {
        variable_manager.variables.insert(argument.field.name.clone(), argument.field.field_type.clone());
    }

    let mut code = CodeStatus::Swapping();
    mem::swap(&mut code, &mut function.code);
    match code {
        CodeStatus::Parsing(parsing) => function.code = CodeStatus::Finished(parsing.await?),
        CodeStatus::Finished(body) => function.code = CodeStatus::Finished(body),
        _ => {}
    }

    verify_code(process_manager, function.code.assume_finished_mut(), syntax, &mut variable_manager).await?;
    return Ok(());
}

#[derive(Clone)]
pub struct CheckerVariableManager {
    pub variables: HashMap<String, Types>
}

impl VariableManager for CheckerVariableManager {
    fn get_variable(&self, name: &String) -> Option<Types> {
        return self.variables.get(name).map(|found| found.clone());
    }
}