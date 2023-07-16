use std::sync::{Arc, Mutex};
use syntax::code::{Effects, ExpressionType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, display_parenless, FinalizedCodeBody, CodelessFinalizedFunction, FunctionData};
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::{CheckerVariableManager, EmptyNameResolver};
use async_recursion::async_recursion;
use syntax::async_getters::ImplementationGetter;
use syntax::async_util::{AsyncDataGetter, NameResolver};
use syntax::code::Effects::NOP;
use syntax::types::FinalizedTypes;
use crate::check_high_level_code::{assign_with_priority, check_args, check_operation, placeholder_error};
use crate::output::TypesChecker;

pub async fn verify_low_code(process_manager: &TypesChecker, resolver: &Box<dyn NameResolver>, code: CodeBody,
                             syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager) -> Result<(bool, FinalizedCodeBody), ParsingError> {
    let mut body = Vec::new();
    for line in code.expressions {
        body.push(FinalizedExpression::new(line.expression_type,
                                           verify_effect(process_manager, resolver.boxed_clone(), line.effect, syntax, variables).await?));
        if let ExpressionType::Return = line.expression_type {
            return Ok((true, FinalizedCodeBody::new(body, code.label.clone(), true)));
        }
    }

    return Ok((false, FinalizedCodeBody::new(body, code.label.clone(), false)));
}

//IntelliJ seems to think the operation loop is unreachable for some reason.
#[allow(unreachable_code)]
#[async_recursion]
async fn verify_effect(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, effect: Effects,
                       syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager) -> Result<FinalizedEffects, ParsingError> {
    let output = match effect.clone() {
        Effects::CodeBody(body) =>
            FinalizedEffects::CodeBody(verify_low_code(process_manager, &resolver, body, syntax, &mut variables.clone()).await?.1),
        Effects::Set(first, second) => {
            FinalizedEffects::Set(Box::new(
                verify_effect(process_manager, resolver.boxed_clone(), *first, syntax, variables).await?),
                                  Box::new(
                                      verify_effect(process_manager, resolver, *second, syntax, variables).await?))
        },
        Effects::Operation(operation, values) => 'outer: {
            let mut args = Vec::new();
            for arg in values {
                args.push(verify_effect(process_manager, resolver.boxed_clone(), arg, syntax, variables).await?);
            }

            let error = ParsingError::new(String::new(), (0, 0), 0,
                                          (0, 0), 0, format!("Failed to find operation {}", operation));
            //Keeps track of the last operation notified of.
            let mut ops = 0;
            loop {
                let operation = format!("{}${}", operation, ops);
                let operations = syntax.lock().unwrap().operations.get(&operation).cloned();
                if let Some(operations) = operations {
                    ops = operations.len();
                    for potential_operation in operations {
                        if let Some(new_effect) = check_operation(
                            AsyncDataGetter::new(syntax.clone(), potential_operation).await, &args,
                            syntax, variables).await? {
                            break 'outer assign_with_priority(new_effect);
                        }
                    }
                }

                Syntax::get_function(syntax.clone(), error.clone(),
                                     operation, true, Box::new(EmptyNameResolver {})).await?;
            }
        },
        Effects::ImplementationCall(calling, traits, method, effects) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables).await?)
            }

            let found = verify_effect(process_manager, resolver.boxed_clone(), *calling, syntax, variables).await?;
            let return_type = found.get_return(variables).unwrap();
            finalized_effects.push(found);

            if let Ok(inner) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                                  traits, resolver.boxed_clone()).await {
                let mut output = None;
                if let Ok(value) = ImplementationGetter::new(syntax.clone(),
                                                             inner.finalize(syntax.clone()).await, return_type.clone()).await {
                    for temp in value {
                        if temp.name == method {
                            output = Some(temp.clone());
                        }
                    }
                }

                check_method(process_manager, AsyncDataGetter::new(syntax.clone(), output.unwrap()).await,
                             finalized_effects, syntax, variables).await?
            } else {
                panic!("Screwed up trait!");
            }
        },
        Effects::MethodCall(calling, method, effects) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables).await?)
            }
            let method = if let Some(found) = calling {
                let found = verify_effect(process_manager, resolver.boxed_clone(), *found, syntax, variables).await?;
                let return_type = found.get_return(variables).unwrap();
                finalized_effects.push(found);
                if let Ok(value) = Syntax::get_function(syntax.clone(), placeholder_error(String::new()),
                                                        method.clone(), false, resolver.boxed_clone()).await {
                    value
                } else {
                    let mut output = None;
                    for import in resolver.imports() {
                        if let Ok(value) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                                              import.clone(), resolver.boxed_clone()).await {
                            if let Ok(value) = ImplementationGetter::new(syntax.clone(),
                                                                         return_type.clone(), value.finalize(syntax.clone()).await).await {
                                for temp in value {
                                    if temp.name == method {
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
                                     method, false, resolver.boxed_clone()).await?
            };

            check_method(process_manager, AsyncDataGetter::new(syntax.clone(), method).await,
                         finalized_effects, syntax, variables).await?
        },
        Effects::CompareJump(effect, first, second) =>
            FinalizedEffects::CompareJump(Box::new(
                verify_effect(process_manager, resolver, *effect, syntax, variables).await?),
                                          first, second),
        Effects::CreateStruct(target, effects) => {
            let mut target = target.finalize(syntax.clone()).await;
            if let FinalizedTypes::GenericType(mut base, mut bounds) = target {
                target = base.flatten(&mut bounds, syntax).await?;
            }
            let mut final_effects = Vec::new();
            for (field_name, effect) in effects {
                let mut i = 0;
                let fields = target.get_fields();
                for field in fields {
                    if field.field.name == field_name {
                        break;
                    }
                    i += 1;
                }

                if i == fields.len() {
                    return Err(placeholder_error(format!("Unknown field {}!", field_name)));
                }

                final_effects.push((i, verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables).await?));
            }

            FinalizedEffects::CreateStruct(target, final_effects)
        },
        Effects::Load(effect, target) => {
            let output = verify_effect(process_manager, resolver, *effect, syntax, variables).await?;

            let types = output.get_return(variables).unwrap().inner_struct().clone();
            FinalizedEffects::Load(Box::new(output), target.clone(), types)
        },
        Effects::CreateVariable(name, effect) => {
            let effect = verify_effect(process_manager, resolver, *effect, syntax, variables).await?;
            let found;
            if let Some(temp_found) = effect.get_return(variables) {
                found = temp_found;
            } else {
                return Err(placeholder_error("No return type!".to_string()));
            };
            variables.variables.insert(name.clone(), found.clone());
            FinalizedEffects::CreateVariable(name.clone(), Box::new(effect), found)
        },
        NOP() => panic!("Tried to compile a NOP!"),
        Effects::Jump(jumping) => FinalizedEffects::Jump(jumping),
        Effects::LoadVariable(variable) => FinalizedEffects::LoadVariable(variable),
        Effects::Float(float) => FinalizedEffects::Float(float),
        Effects::Int(int) => FinalizedEffects::UInt(int as u64),
        Effects::UInt(uint) => FinalizedEffects::UInt(uint),
        Effects::Bool(bool) => FinalizedEffects::Bool(bool),
        Effects::String(string) => FinalizedEffects::String(string)
    };
    return Ok(output);
}

async fn check_method(process_manager: &TypesChecker, mut method: Arc<CodelessFinalizedFunction>,
                      effects: Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                      variables: &mut CheckerVariableManager) -> Result<FinalizedEffects, ParsingError> {
    if !method.generics.is_empty() {
        let mut manager = process_manager.clone();

        for i in 0..method.fields.len() {
            let effect = effects.get(i).unwrap().get_return(variables).unwrap();
            if let Some(old) = method.fields.get(i).unwrap().field.field_type.resolve_generic(
                &effect, syntax, placeholder_error("Invalid bounds!".to_string())).await? {
                if let FinalizedTypes::Generic(name, _) = old {
                    manager.generics.insert(name, effect);
                } else {
                    panic!("Guh?");
                }
            }
        }

        let name = format!("{}_{}", method.data.name, display_parenless(
            &manager.generics.values().collect(), "_"));
        {
            if syntax.lock().unwrap().functions.types.contains_key(&name) {
                let data = syntax.lock().unwrap().functions.types.get(&name).unwrap().clone();
                method = AsyncDataGetter::new(syntax.clone(), data).await;
            } else {
                let mut new_method = CodelessFinalizedFunction::clone(&method);
                new_method.generics.clear();
                let mut method_data = FunctionData::clone(&method.data);
                method_data.name = name.clone();
                new_method.data = Arc::new(method_data);
                for field in &mut new_method.fields {
                    field.field.field_type.degeneric(&manager.generics, syntax,
                                                     placeholder_error("No generic!".to_string()),
                                                     placeholder_error("Invalid bounds!".to_string())).await?;
                }

                if let Some(returning) = &mut new_method.return_type {
                    returning.degeneric(&manager.generics, syntax,
                                        placeholder_error("No generic!".to_string()),
                                        placeholder_error("Invalid bounds!".to_string())).await?;
                }
                method = Arc::new(new_method);
                let mut locked = syntax.lock().unwrap();
                if let Some(wakers) = locked.functions.wakers.remove(&name) {
                    for waker in wakers {
                        waker.wake();
                    }
                }

                locked.functions.types.insert(name, method.data.clone());
                locked.functions.data.insert(method.data.clone(), method.clone());
            };
        }

        let temp_effect = FinalizedEffects::MethodCall(method.clone(), effects);
        return Ok(temp_effect);
    }

    if !check_args(&method, &effects, syntax, variables).await? {
        return Err(placeholder_error(format!("Incorrect args to method {}: {:?} vs {:?}", method.data.name,
                                             method.fields.iter().map(|field| &field.field.field_type).collect::<Vec<_>>(),
                                             effects.iter().map(|effect| effect.get_return(variables).unwrap()).collect::<Vec<_>>())));
    }

    return Ok(FinalizedEffects::MethodCall(method, effects));
}