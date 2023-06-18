use std::mem;
use std::sync::{Arc, Mutex};
use syntax::code::{Effects, ExpressionType};
use syntax::function::{CodeBody, display_parenless, Function};
use syntax::{Attribute, ParsingError};
use syntax::syntax::Syntax;
use crate::EmptyNameResolver;
use async_recursion::async_recursion;
use syntax::async_getters::ImplementationGetter;
use syntax::async_util::NameResolver;
use syntax::types::Types;
use crate::check_function::CheckerVariableManager;
use crate::output::TypesChecker;

pub async fn verify_code(process_manager: &TypesChecker, resolver: &Box<dyn NameResolver>, code: &mut CodeBody,
                         syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager) -> Result<bool, ParsingError> {
    for line in &mut code.expressions {
        verify_effect(process_manager, resolver.boxed_clone(), &mut line.effect, syntax, variables).await?;
        if let ExpressionType::Return = line.expression_type {
            return Ok(true);
        }
    }

    return Ok(false);
}

#[async_recursion]
async fn verify_effect(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, effect: &mut Effects,
                       syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager) -> Result<(), ParsingError> {
    match effect {
        Effects::CodeBody(body) => {
            verify_code(process_manager, &resolver, body, syntax, &mut variables.clone()).await?;
        }
        Effects::Set(first, second) => {
            verify_effect(process_manager, resolver.boxed_clone(), first, syntax, variables).await?;
            verify_effect(process_manager, resolver, second, syntax, variables).await?;
        }
        Effects::Operation(operation, values) => {
            for arg in &mut *values {
                verify_effect(process_manager, resolver.boxed_clone(), arg, syntax, variables).await?;
            }

            let error = ParsingError::new(String::new(), (0, 0), 0,
                                          (0, 0), 0, format!("Failed to find operation {}", operation));
            //Keeps track of the last operation notified of.
            let mut ops = 0;
            'outer: loop {
                let operation = format!("{}${}", operation, ops);
                let operations = syntax.lock().unwrap().operations.get(&operation).cloned();
                if let Some(operations) = operations {
                    ops = operations.len();
                    for potential_operation in operations {
                        if let Some(new_effect) = check_operation(potential_operation, values,
                                                                  syntax, variables).await? {
                            *effect = assign_with_priority(new_effect);
                            break 'outer;
                        }
                    }
                }

                Syntax::get_function(syntax.clone(), error.clone(),
                                     operation, true, Box::new(EmptyNameResolver {})).await?;
            }
            return verify_effect(process_manager, resolver, effect, syntax, variables).await;
        }
        Effects::MethodCall(calling, method, effects) => {
            let mut method = if let Some(found) = calling {
                effects.push(*found.clone());
                let return_type = found.get_return(variables).unwrap();
                if let Ok(value) = Syntax::get_function(syntax.clone(), placeholder_error(String::new()),
                                                        method.clone(), false, resolver.boxed_clone()).await {
                    value
                } else {
                    let mut output = None;
                    for import in resolver.imports() {
                        if let Ok(value) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                                              import.clone(), resolver.boxed_clone()).await {
                            while let Ok(value) = ImplementationGetter::new(syntax.clone(),
                                                                            return_type.clone(), value.clone()).await {
                                for temp in value {
                                    if &temp.name == method {
                                        if output.is_some() {
                                            return Err(placeholder_error(format!("Ambiguous method {}", method)));
                                        }
                                        output = Some(temp.clone());
                                    }
                                }
                            }
                        }
                    }
                    if let Some(value) = output {
                        value
                    } else {
                        return Err(placeholder_error(format!("Unknown method {}", method)));
                    }
                }
            } else {
                Syntax::get_function(syntax.clone(), placeholder_error(format!("Unknown method {}", method)),
                                     method.clone(), false, resolver.boxed_clone()).await?
            };

            if let Some(found) = check_method(process_manager, resolver, &mut method,
                                              effects, syntax, variables).await? {
                *effect = found;
            }
        }
        Effects::CompareJump(effect, _, _) => verify_effect(process_manager, resolver, effect, syntax, variables).await?,
        Effects::CreateStruct(target, effects) => {
            if let Types::GenericType(base, bounds) = target {
                *target = base.flatten(bounds, syntax).await?;
            }
            for (_, effect) in effects {
                verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables).await?;
            }
        }
        Effects::Load(effect, _) => verify_effect(process_manager, resolver, effect, syntax, variables).await?,
        Effects::CreateVariable(name, effect) => {
            verify_effect(process_manager, resolver, effect, syntax, variables).await?;
            return if let Some(found) = effect.get_return(variables) {
                variables.variables.insert(name.clone(), found);
                Ok(())
            } else {
                Err(placeholder_error("No return type!".to_string()))
            };
        }
        _ => {}
    }
    return Ok(());
}

async fn check_method(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, method: &mut Arc<Function>,
                      effects: &mut Vec<Effects>, syntax: &Arc<Mutex<Syntax>>,
                      variables: &mut CheckerVariableManager) -> Result<Option<Effects>, ParsingError> {
    if !method.generics.is_empty() {
        let mut manager = process_manager.clone();

        for effect in &mut *effects {
            verify_effect(&mut manager, resolver.boxed_clone(), effect, syntax, variables).await?;
        }

        for i in 0..method.fields.len() {
            let effect = effects.get(i).unwrap().get_return(variables).unwrap();
            if let Some(old) = unsafe { Arc::get_mut_unchecked(method) }.fields.get_mut(i)
                .unwrap().await_finish().await?.field.field_type.resolve_generic(
                &effect, syntax, placeholder_error("Invalid bounds!".to_string())).await? {
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
            if syntax.lock().unwrap().functions.types.contains_key(&name) {
                *method = syntax.lock().unwrap().functions.types.get(&name).unwrap().clone()
            } else {
                let mut new_method = Function::clone(method);
                new_method.generics.clear();
                new_method.name = name.clone();
                for field in &mut new_method.fields {
                    field.assume_finished_mut().field.field_type.degeneric(&manager.generics, syntax,
                                                                           placeholder_error("No generic!".to_string()),
                                                                           placeholder_error("Invalid bounds!".to_string())).await?;
                }
                if let Some(returning) = &mut new_method.return_type {
                    returning.assume_finished_mut().degeneric(&manager.generics, syntax,
                                                              placeholder_error("No generic!".to_string()),
                                                              placeholder_error("Invalid bounds!".to_string())).await?;
                }
                *method = Arc::new(new_method);
                syntax.lock().unwrap().functions.types.insert(name, method.clone());
            };
        }

        let mut temp_effect = Effects::VerifiedMethodCall(method.clone(), temp);
        verify_effect(&mut manager, resolver, &mut temp_effect, syntax, variables).await?;
        unsafe { Arc::get_mut_unchecked(&mut method.clone()) }.generics.clear();
        return Ok(Some(temp_effect));
    }

    for effect in &mut *effects {
        verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables).await?;
    }

    if !check_args(&method, effects, syntax, variables).await? {
        return Err(placeholder_error(format!("Incorrect args to method {}: {:?} vs {:?}", method.name,
                                             method.fields.iter().map(|field| &field.assume_finished().field.field_type).collect::<Vec<_>>(),
                                             effects.iter().map(|effect| effect.get_return(variables).unwrap()).collect::<Vec<_>>())));
    }

    return Ok(None);
}

pub fn placeholder_error(message: String) -> ParsingError {
    return ParsingError::new("".to_string(), (0, 0), 0, (0, 0), 0, message);
}

async fn check_operation(operation: Arc<Function>, values: &Vec<Effects>, syntax: &Arc<Mutex<Syntax>>,
                         variables: &mut CheckerVariableManager) -> Result<Option<Effects>, ParsingError> {
    if check_args(&operation, values, syntax, variables).await? {
        return Ok(Some(Effects::VerifiedMethodCall(operation, values.clone())));
    }
    return Ok(None);
}

async fn check_args(function: &Arc<Function>, args: &Vec<Effects>, syntax: &Arc<Mutex<Syntax>>,
                    variables: &mut CheckerVariableManager) -> Result<bool, ParsingError> {
    if function.fields.len() != args.len() {
        return Ok(false);
    }

    let mut function = function.clone();
    for i in 0..function.fields.len() {
        let returning = args.get(i).unwrap().get_return(variables);
        if returning.is_some() && !returning.as_ref().unwrap().of_type(
            &unsafe { Arc::get_mut_unchecked(&mut function) }.fields.get_mut(i).unwrap().await_finish().await?.field.field_type, syntax).await {
            println!("{} != {}", returning.as_ref().unwrap(), function.fields.get(i).unwrap().assume_finished().field.field_type);
            return Ok(false);
        }
    }

    return Ok(true);
}

pub fn assign_with_priority(operator: Effects) -> Effects {
    //Needs ownership of the value
    let (func, mut effects) = if let Effects::VerifiedMethodCall(func, effects) = operator {
        (func, effects)
    } else {
        panic!("If your seeing this, something went VERY wrong");
    };
    if effects.len() != 2 {
        return Effects::VerifiedMethodCall(func, effects);
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
        Effects::VerifiedMethodCall(lhs_func, mut lhs) => {
            // temp_lhs = (1 + 2), operator = {} / 2
            if lhs_priority < op_priority || (!op_parse_left && lhs_priority == op_priority) {
                // temp_lhs = 1 + {}, operator = 2 / 2
                mem::swap(lhs.last_mut().unwrap(), effects.first_mut().unwrap());

                // 1 + (2 / 2)
                mem::swap(lhs.last_mut().unwrap(), &mut Effects::VerifiedMethodCall(func, effects));

                return Effects::VerifiedMethodCall(lhs_func.clone(), lhs);
            } else {
                effects.insert(0, Effects::VerifiedMethodCall(lhs_func.clone(), lhs));
            }
        }
        _ => effects.insert(0, lhs)
    }

    return Effects::VerifiedMethodCall(func, effects);
}