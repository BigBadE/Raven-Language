use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use syntax::function::Function;
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

    for argument in &mut function.fields {
        let field = argument.await_finish().await?;
        variable_manager.variables.insert(field.field.name.clone(),
                                          field.field.field_type.clone());
    }

    function.code.await_finish().await?;

    println!("{}: {:?}", function.name, function.code.assume_finished().expressions);
    if !verify_code(process_manager, &resolver, function.code.assume_finished_mut(), syntax, &mut variable_manager).await? {
        if function.return_type.is_none() {
            function.code.assume_finished_mut().expressions.push(Expression::new(ExpressionType::Return, Effects::NOP()));
        } else {
            return Err(placeholder_error(format!("Function {} doesn't return a {}!", function.name,
                                                 function.return_type.as_ref().unwrap().assume_finished())))
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