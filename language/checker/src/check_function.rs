use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex};
use syntax::function::{CodeStatus, Function};
use syntax::{is_modifier, Modifier, ParsingError, VariableManager};
use syntax::async_util::NameResolver;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_code::{placeholder_error, verify_code};
use crate::output::TypesChecker;

pub async fn verify_function(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, function: &mut Function, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
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

    if !verify_code(process_manager, &resolver, function.code.assume_finished_mut(), syntax, &mut variable_manager).await? {
        if function.return_type.is_none() {
            function.code.assume_finished_mut().expressions.push(Expression::new(ExpressionType::Return, Effects::NOP()));
        } else {
            return Err(placeholder_error(format!("Function doesn't return a {}!",
                                                 function.return_type.as_ref().unwrap())))
        }
    }
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