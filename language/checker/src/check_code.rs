use std::sync::{Arc, Mutex};
use syntax::code::Effects;
use syntax::function::{CodeBody, Function};
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::EmptyNameResolver;
use async_recursion::async_recursion;

pub async fn verify_code(code: &mut CodeBody, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    for line in &mut code.expressions {
        verify_effect(&mut line.effect, syntax).await?;
    }
    return Ok(());
}

#[async_recursion]
async fn verify_effect(effect: &mut Effects, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    match effect {
        Effects::CodeBody(body) => verify_code(body, syntax).await?,
        Effects::Set(first, second) => {
            verify_effect(first, syntax).await?;
            verify_effect(second, syntax).await?;
        },
        Effects::Operation(operation, values) => {
            {
                let locked = syntax.lock().unwrap();
                if let Some(operations) = locked.operations.get(operation) {
                    for potential_operation in operations {
                        if let Some(new_effect) = check_operation(potential_operation, values) {
                            *effect = new_effect;
                            return Ok(());
                        }
                    }
                }
            }

            loop {
                let func = Syntax::get_function(syntax.clone(),
                                     ParsingError::new(String::new(), (0, 0), 0,
                                                       (0, 0), 0, "Temp".to_string()),
                                     operation.clone(), Box::new(EmptyNameResolver {})).await?;
                if let Some(new_effect) = check_operation(&func, values) {
                    *effect = new_effect;
                    return Ok(());
                }
            }
        },
        Effects::MethodCall(_, effects) => for effect in effects {
            verify_effect(effect, syntax).await?;
        },
        Effects::CompareJump(effect, _, _) => verify_effect(effect, syntax).await?,
        Effects::CreateStruct(_, effects) => for (_, effect) in effects {
            verify_effect(effect, syntax).await?;
        },
        Effects::Load(effect, _) => verify_effect(effect, syntax).await?,
        _ => {}
    }
    return Ok(());
}

fn check_operation(operation: &Arc<Function>, values: &Vec<Effects>) -> Option<Effects> {
    if check_args(operation, values) {
        return Some(Effects::MethodCall(operation.clone(), values.clone()));
    }
    return None;
}

fn check_args(function: &Arc<Function>, args: &Vec<Effects>) -> bool {
    todo!()
}