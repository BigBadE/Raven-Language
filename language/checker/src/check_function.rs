use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex};
use syntax::function::{CodelessFinalizedFunction, FinalizedCodeBody, FinalizedFunction, UnfinalizedFunction};
use syntax::{is_modifier, Modifier, ParsingError};
use syntax::async_util::NameResolver;
use syntax::code::{ExpressionType, FinalizedEffects, FinalizedExpression, FinalizedField, FinalizedMemberField};
use syntax::syntax::Syntax;
use syntax::types::FinalizedTypes;
use crate::check_code::{placeholder_error, verify_code};
use crate::{CheckerVariableManager, finalize_generics};
use crate::output::TypesChecker;

pub async fn verify_function(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>,
                             mut function: UnfinalizedFunction, syntax: &Arc<Mutex<Syntax>>) -> Result<FinalizedFunction, ParsingError> {
    let mut variable_manager = CheckerVariableManager { variables: HashMap::new() };

    let mut fields = Vec::new();
    for argument in &mut function.fields {
        let field = argument.await?;
        let mut field = FinalizedMemberField { modifiers: field.modifiers, attributes: field.attributes,
            field: FinalizedField { field_type: field.field.field_type.finalize(syntax.clone()).await, name: field.field.name } };
        variable_manager.variables.insert(field.field.name.clone(),
                                          field.field.field_type.clone());
        //if !field.field.field_type.is_primitive() {
            let mut temp = FinalizedTypes::Generic(String::new(), vec!());
            mem::swap(&mut temp, &mut field.field.field_type);
            field.field.field_type = FinalizedTypes::Reference(Box::new(temp));
        //}
        fields.push(field);
    }

    let return_type = if let Some(return_type) = function.return_type.as_mut() {
        let first = match return_type.await {
            Ok(result) => result,
            Err(error) => {
                println!("Found dumb error: {}", error);
                return Err(error);
            }
        };

        Some(first.finalize(syntax.clone()).await)
    } else {
        None
    };

    let codeless = CodelessFinalizedFunction {
        generics: finalize_generics(syntax, function.generics).await?,
        fields,
        return_type,
        data: function.data.clone(),
    };

    {
        let mut locked = syntax.lock().unwrap();
        if let Some(wakers) = locked.functions.wakers.remove(&function.data.name) {
            for waker in wakers {
                waker.wake();
            }
        }
        locked.functions.data.insert(function.data.clone(), Arc::new(codeless.clone()));
    }

    //Internal/external/trait functions verify everything but the code.
    if is_modifier(function.data.modifiers, Modifier::Internal) || is_modifier(function.data.modifiers, Modifier::Extern) {
        return Ok(codeless.clone().add_code(FinalizedCodeBody::new(Vec::new(), String::new(), true)));
    }

    let mut code_output = verify_code(process_manager, &resolver, function.code.await?, syntax, &mut variable_manager).await?;
    if !code_output.0 {
        if function.return_type.is_none() {
            code_output.1.expressions.push(FinalizedExpression::new(ExpressionType::Return, FinalizedEffects::NOP()));
        } else if is_modifier(function.data.modifiers, Modifier::Trait) {
            return Err(placeholder_error(format!("Function {} doesn't return a {}!", function.data.name,
                                                 codeless.return_type.as_ref().unwrap())));
        }
    }

    return Ok(codeless.clone().add_code(code_output.1));
}