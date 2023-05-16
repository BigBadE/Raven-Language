use std::mem;
use std::sync::{Arc, Mutex};
use syntax::code::Effects;
use syntax::function::{CodeBody, display_parenless, Function};
use syntax::{Attribute, ParsingError};
use syntax::syntax::Syntax;
use crate::EmptyNameResolver;
use async_recursion::async_recursion;
use syntax::types::Types;
use crate::check_function::CheckerVariableManager;
use crate::output::TypesChecker;

pub async fn verify_code(process_manager: &TypesChecker, code: &mut CodeBody,
                         syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager) -> Result<(), ParsingError> {
    for line in &mut code.expressions {
        verify_effect(process_manager, &mut line.effect, syntax, variables).await?;
    }
    return Ok(());
}

#[async_recursion]
async fn verify_effect(process_manager: &TypesChecker, effect: &mut Effects, syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager) -> Result<(), ParsingError> {
    println!("Matching {:?}", effect);
    match effect {
        Effects::CodeBody(body) => verify_code(process_manager, body, syntax, &mut variables.clone()).await?,
        Effects::Set(first, second) => {
            verify_effect(process_manager, first, syntax, variables).await?;
            verify_effect(process_manager, second, syntax, variables).await?;
        }
        Effects::Operation(operation, values) => {
            for arg in &mut *values {
                verify_effect(process_manager, arg, syntax, variables).await?;
            }

            let error = ParsingError::new(String::new(), (0, 0), 0,
                                          (0, 0), 0, format!("Failed to find operation {}", operation));
            //Keeps track of the last operation notified of.
            let mut ops = 0;
            'outer: loop {
                let operation = format!("{}${}", operation, ops);
                {
                    let locked = syntax.lock().unwrap();
                    if let Some(operations) = locked.operations.get(&operation) {
                        ops = operations.len();
                        for potential_operation in operations {
                            if let Some(new_effect) = check_operation(potential_operation, values, variables) {
                                *effect = assign_with_priority(new_effect);
                                break 'outer;
                            }
                        }
                    }
                }

                Syntax::get_function(syntax.clone(), error.clone(),
                                     operation, true, Box::new(EmptyNameResolver {})).await?;
            }
            return verify_effect(process_manager, effect, syntax, variables).await;
        }
        Effects::MethodCall(method, effects) => {
            if !method.generics.is_empty() {
                let mut manager = process_manager.clone();

                for effect in &mut *effects {
                    verify_effect(&mut manager, effect, syntax, variables).await?;
                }

                for i in 0..method.fields.len() {
                    let effect = effects.get(i).unwrap().get_return(variables).unwrap();
                    if let Some(old) = method.fields.get(i).unwrap().field.field_type.resolve_generic(
                        &effect, placeholder_error("Invalid bounds!".to_string()))? {
                        if let Types::Generic(name, _) = old {
                            manager.generics.insert(name, effect);
                        } else {
                            panic!("Guh?");
                        }
                    }
                }

                let name = format!("{}_{}", method.name, display_parenless(
                    &manager.generics.values().collect(), "_"));
                let mut temp = Vec::new();
                mem::swap(&mut temp, effects);
                {
                    let mut locked = syntax.lock().unwrap();
                    if let Some(found) = locked.functions.types.get(&name) {
                        *method = found.clone()
                    } else {
                        let mut new_method = Function::clone(method);
                        new_method.generics.clear();
                        new_method.name = name.clone();
                        for field in &mut new_method.fields {
                            field.field.field_type.degeneric(&manager.generics,
                                             placeholder_error("No generic!".to_string()),
                                             placeholder_error("Invalid bounds!".to_string()))?;
                        }
                        if let Some(returning) = &mut new_method.return_type {
                            returning.degeneric(&manager.generics,
                                                placeholder_error("No generic!".to_string()),
                                                placeholder_error("Invalid bounds!".to_string()))?;
                        }
                        *method = Arc::new(new_method);
                        locked.functions.types.insert(name, method.clone());
                    };
                }

                let mut temp_effect = Effects::MethodCall(method.clone(), temp);
                verify_effect(&mut manager, &mut temp_effect, syntax, variables).await?;
                unsafe { Arc::get_mut_unchecked(&mut method.clone()) }.generics.clear();
                *effect = temp_effect;
                return Ok(());
            }

            for effect in &mut *effects {
                verify_effect(process_manager, effect, syntax, variables).await?;
            }

            if !check_args(&method, effects, variables) {
                return Err(placeholder_error(format!("Incorrect args to method {}", method.name)));
            }
        }
        Effects::CompareJump(effect, _, _) => verify_effect(process_manager, effect, syntax, variables).await?,
        Effects::CreateStruct(target, effects) => {
            if let Types::GenericType(base, bounds) = target {
                *target = base.flatten(bounds, syntax);
            }
            for (_, effect) in effects {
                verify_effect(process_manager, effect, syntax, variables).await?;
            }
        },
        Effects::Load(effect, _) => verify_effect(process_manager, effect, syntax, variables).await?,
        Effects::CreateVariable(name, effect) => {
            verify_effect(process_manager, effect, syntax, variables).await?;
            return if let Some(found) = effect.get_return(variables) {
                variables.variables.insert(name.clone(), found);
                Ok(())
            } else {
                Err(placeholder_error("No return type!".to_string()))
            }
        },
        _ => {}
    }
    return Ok(());
}

pub fn placeholder_error(message: String) -> ParsingError {
    return ParsingError::new("".to_string(), (0, 0), 0, (0, 0), 0, message);
}

fn check_operation(operation: &Arc<Function>, values: &Vec<Effects>,
                   variables: &mut CheckerVariableManager) -> Option<Effects> {
    if check_args(operation, values, variables) {
        return Some(Effects::MethodCall(operation.clone(), values.clone()));
    }
    return None;
}

fn check_args(function: &Arc<Function>, args: &Vec<Effects>, variables: &mut CheckerVariableManager) -> bool {
    if function.fields.len() != args.len() {
        return false;
    }

    for i in 0..function.fields.len() {
        let returning = args.get(i).unwrap().get_return(variables);
        if returning.is_some() && !function.fields.get(i).unwrap().field.field_type.of_type(
            returning.as_ref().unwrap()) {
            println!("{} != {}", function.fields.get(i).unwrap().field.field_type, returning.as_ref().unwrap());
            return false;
        }
    }

    return true;
}

pub fn assign_with_priority(operator: Effects) -> Effects {
    //Needs ownership of the value
    let (func, mut effects) = if let Effects::MethodCall(func, effects) = operator {
        (func, effects)
    } else {
        panic!("If your seeing this, something went VERY wrong");
    };
    if effects.len() != 2 {
        return Effects::MethodCall(func, effects);
    }

    let op_priority = match Attribute::find_attribute("priority", &func.attributes) {
        Some(found) => match found {
            Attribute::Integer(_, priority) => *priority,
            _ => 0,
        },
        None => 0
    };

    let op_parse_left = match Attribute::find_attribute("parse_left", &func.attributes) {
        Some(found) => match found {
            Attribute::Bool(_, priority) => *priority,
            _ => true,
        },
        None => true
    };

    let lhs = effects.remove(0);

    let lhs_priority = match Attribute::find_attribute("priority", &func.attributes) {
        Some(found) => match found {
            Attribute::Integer(_, priority) => *priority,
            _ => 0,
        },
        None => 0
    };

    match lhs {
        // Code explained using the following example: 1 + 2 / 2
        Effects::MethodCall(lhs_func, mut lhs) => {
            // temp_lhs = (1 + 2), operator = {} / 2
            if lhs_priority < op_priority || (!op_parse_left && lhs_priority == op_priority) {
                // temp_lhs = 1 + {}, operator = 2 / 2
                mem::swap(lhs.last_mut().unwrap(), effects.first_mut().unwrap());

                // 1 + (2 / 2)
                mem::swap(lhs.last_mut().unwrap(), &mut Effects::MethodCall(func, effects));

                return Effects::MethodCall(lhs_func.clone(), lhs);
            } else {
                effects.insert(0, Effects::MethodCall(lhs_func.clone(), lhs));
            }
        }
        _ => effects.insert(0, lhs)
    }

    return Effects::MethodCall(func, effects);
}