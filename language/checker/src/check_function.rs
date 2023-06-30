use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex};
use syntax::function::Function;
use syntax::{is_modifier, Modifier, ParsingError, VariableManager};
use syntax::async_util::NameResolver;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::check_code::{placeholder_error, verify_code};
use crate::output::TypesChecker;

pub async fn verify_function(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>,
                             function: &mut Function, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    let mut variable_manager = CheckerVariableManager { variables: HashMap::new() };

    for argument in &mut function.fields {
        let field = argument.await_finish().await?;
        variable_manager.variables.insert(field.field.name.clone(),
                                          field.field.field_type.clone());
        if !field.field.field_type.is_primitive() {
            let mut temp = Types::Generic(String::new(), vec!());
            mem::swap(&mut temp, &mut field.field.field_type);
            field.field.field_type = Types::Reference(Box::new(temp));
        }
    }

    if let Some(return_type) = function.return_type.as_mut() {
        return_type.await_finish().await?;
    }

    //Internal functions verify everything but the code.
    if is_modifier(function.modifiers, Modifier::Internal) {
        function.code.await_finish().await?;
        return Ok(());
    }

    if !verify_code(process_manager, &resolver, function.code.await_finish().await?, syntax, &mut variable_manager).await? {
        if function.return_type.is_none() {
            function.code.assume_finished_mut().expressions.push(Expression::new(ExpressionType::Return, Effects::NOP()));
        } else if is_modifier(function.modifiers, Modifier::Trait) {
            return Err(placeholder_error(format!("Function {} doesn't return a {}!", function.name,
                                                 function.return_type.as_ref().unwrap().assume_finished())))
        }
    }
    return Ok(());
}

#[derive(Clone, Debug)]
pub struct CheckerVariableManager {
    pub variables: HashMap<String, Types>
}

impl VariableManager for CheckerVariableManager {
    fn get_variable(&self, name: &String) -> Option<Types> {
        return self.variables.get(name).map(|found| found.clone());
    }
}