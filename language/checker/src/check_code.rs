use std::sync::Arc;
use std::sync::Mutex;
use syntax::code::{Effects, ExpressionType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, FinalizedCodeBody, CodelessFinalizedFunction};
use syntax::{Attribute, SimpleVariableManager, is_modifier, Modifier, ParsingError};
use syntax::syntax::Syntax;
use async_recursion::async_recursion;
use syntax::async_util::{AsyncDataGetter, NameResolver};
use syntax::operation_util::OperationGetter;
use syntax::r#struct::{StructData, VOID};
use syntax::top_element_manager::{ImplWaiter, TraitImplWaiter};
use syntax::types::FinalizedTypes;
use crate::output::TypesChecker;

pub async fn verify_code(process_manager: &TypesChecker, resolver: &Box<dyn NameResolver>, code: CodeBody,
                         syntax: &Arc<Mutex<Syntax>>, variables: &mut SimpleVariableManager, references: bool, top: bool) -> Result<FinalizedCodeBody, ParsingError> {
    let mut body = Vec::new();
    let mut found_end = false;
    for line in code.expressions {
        match &line.effect {
            Effects::CompareJump(_, _, _) => found_end = true,
            Effects::Jump(_) => found_end = true,
            _ => {}
        }

        body.push(FinalizedExpression::new(line.expression_type,
                                           verify_effect(process_manager, resolver.boxed_clone(),
                                                         line.effect, syntax, variables, references).await?));

        if let ExpressionType::Return = line.expression_type {
            return Ok(FinalizedCodeBody::new(body, code.label.clone(), true));
        }
    }

    if !found_end && !top {
        panic!("Code body with label {} doesn't return or jump!", code.label)
    }

    return Ok(FinalizedCodeBody::new(body, code.label.clone(), false));
}

#[async_recursion]
async fn verify_effect(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, effect: Effects,
                       syntax: &Arc<Mutex<Syntax>>, variables: &mut SimpleVariableManager, references: bool) -> Result<FinalizedEffects, ParsingError> {
    let output = match effect {
        Effects::Paren(inner) => verify_effect(process_manager, resolver, *inner, syntax, variables, references).await?,
        Effects::CodeBody(body) =>
            FinalizedEffects::CodeBody(verify_code(process_manager, &resolver, body, syntax, &mut variables.clone(), references, false).await?),
        Effects::Set(first, second) => {
            FinalizedEffects::Set(Box::new(
                verify_effect(process_manager, resolver.boxed_clone(), *first, syntax, variables, references).await?),
                                  Box::new(
                                      verify_effect(process_manager, resolver, *second, syntax, variables, references).await?))
        }
        Effects::Operation(operation, mut values) => {
            let error = ParsingError::new(String::new(), (0, 0), 0,
                                          (0, 0), 0, format!("Failed to find operation {} with {:?}", operation, values));
            let mut outer_operation = None;
            // Check if it's two operations that should be combined, like a list ([])
            if values.len() > 0 {
                let mut reading_array = None;
                let mut last = values.pop().unwrap();
                if let Effects::CreateArray(mut effects) = last {
                    if effects.len() > 0 {
                        last = effects.pop().unwrap();
                        reading_array = Some(effects);
                    } else {
                        last = Effects::CreateArray(vec!());
                    }
                }

                if let Effects::Operation(inner_operation, effects) = last {
                    if operation.ends_with("{}") && inner_operation.starts_with("{}") {
                        let combined =
                            operation[0..operation.len() - 2].to_string() + &inner_operation;
                        let new_operation = if operation.starts_with("{}") && inner_operation.ends_with("{}") {
                            let mut output = vec!();
                            for i in 0..combined.len() - operation.len() - 2 {
                                let mut temp = combined.clone();
                                temp.truncate(operation.len() + i);
                                output.push(temp);
                            }
                            output
                        } else {
                            vec!(combined.clone())
                        };

                        let getter = OperationGetter {
                            syntax: syntax.clone(),
                            operation: new_operation.clone(),
                            error: error.clone(),
                        };

                        if let Ok(found) = getter.await {
                            let new_operation = Attribute::find_attribute("operation", &found.attributes).unwrap().as_string_attribute().unwrap();

                            let mut inner_array = false;
                            if let Some(found) = reading_array {
                                values.push(Effects::CreateArray(found));
                                inner_array = true;
                            }
                            if new_operation.len() >= combined.len() {
                                if inner_array {
                                    if let Effects::CreateArray(last) = values.last_mut().unwrap() {
                                        for effect in effects {
                                            last.push(effect);
                                        }
                                    }
                                } else {
                                    for effect in effects {
                                        values.push(effect);
                                    }
                                }
                                outer_operation = Some(found);
                            } else {
                                let new_inner = "{}".to_string() + &combined[new_operation.replace("{+}", "{}").len()..];

                                let inner_data = OperationGetter {
                                    syntax: syntax.clone(),
                                    operation: vec!(new_inner.clone()),
                                    error: error.clone(),
                                }.await?;

                                (outer_operation, values) = assign_with_priority(new_operation.clone(), &found, values,
                                                                                 new_inner, &inner_data, effects, inner_array);
                            }
                        } else {
                            if let Some(mut found) = reading_array {
                                if let Effects::CreateArray(inner) = found.last_mut().unwrap() {
                                    inner.push(Effects::Operation(inner_operation, effects));
                                } else {
                                    panic!("Expected array!");
                                }
                            } else {
                                let outer_data = OperationGetter {
                                    syntax: syntax.clone(),
                                    operation: vec!(operation.clone()),
                                    error: error.clone(),
                                }.await?;
                                let inner_data = OperationGetter {
                                    syntax: syntax.clone(),
                                    operation: vec!(inner_operation.clone()),
                                    error: error.clone(),
                                }.await?;

                                (outer_operation, values) = assign_with_priority(operation.clone(), &outer_data, values,
                                                                                 inner_operation, &inner_data, effects, false);
                            }
                        }
                    } else {
                        if let Some(mut found) = reading_array {
                            if let Effects::CreateArray(inner) = found.last_mut().unwrap() {
                                inner.push(Effects::Operation(inner_operation, effects));
                            } else {
                                panic!("Expected array!");
                            }
                        } else {
                            values.push(Effects::Operation(inner_operation, effects));
                        }
                    }
                } else {
                    if let Some(mut found) = reading_array {
                        if let Effects::CreateArray(inner) = found.last_mut().unwrap() {
                            inner.push(last);
                        } else {
                            panic!("Expected array!");
                        }
                    } else {
                        values.push(last);
                    }
                }
            }

            let operation = if let Some(found) = outer_operation {
                found
            } else {
                OperationGetter {
                    syntax: syntax.clone(),
                    operation: vec!(operation),
                    error,
                }.await?
            };

            if Attribute::find_attribute("operation", &operation.attributes).unwrap().as_string_attribute().unwrap().contains("{+}") {
                if let Effects::CreateArray(_) = values.get(0).unwrap() {} else {
                    let effect = Effects::CreateArray(vec!(values.remove(0)));
                    values.push(effect);
                }
            }

            let calling;
            if values.len() > 0 {
                calling = Box::new(values.remove(0));
            } else {
                calling = Box::new(Effects::NOP());
            }

            verify_effect(process_manager, resolver,
                          Effects::ImplementationCall(calling, operation.name.clone(),
                                                      String::new(), values, None),
                          syntax, variables, references).await?
        }
        Effects::ImplementationCall(calling, traits, method, effects, returning) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables, references).await?)
            }

            let return_type;
            if let Effects::NOP() = *calling {
                return_type = FinalizedTypes::Struct(VOID.clone());
            } else {
                let found = verify_effect(process_manager, resolver.boxed_clone(), *calling, syntax, variables, references).await?;
                return_type = found.get_return(variables).unwrap();
                finalized_effects.insert(0, found);
            }

            if let Ok(inner) = Syntax::get_struct(syntax.clone(), ParsingError::empty(),
                                                  traits.clone(), resolver.boxed_clone(), vec!()).await {
                let data = inner.finalize(syntax.clone()).await;
                if return_type.of_type_sync(&data, None).0 {
                    let mut i = 0;
                    for found in &data.inner_struct().data.functions {
                        if found.name == method {
                            return Ok(FinalizedEffects::VirtualCall(i,
                                                                    AsyncDataGetter::new(syntax.clone(), found.clone()).await,
                                                                    finalized_effects));
                        }
                        i += 1;
                    }

                    if !method.is_empty() {
                        return Err(placeholder_error(
                            format!("Unknown method {} in {}", method, data)));
                    }
                }

                let try_get_impl = async || -> Result<Option<FinalizedEffects>, ParsingError> {
                    let result = ImplWaiter {
                        syntax: syntax.clone(),
                        return_type: return_type.clone(),
                        data: data.clone(),
                        error: placeholder_error(
                            format!("Nothing implements {} for {}", inner, return_type)),
                    }.await?;

                    for temp in &result {
                        if temp.name == method || method.is_empty() {
                            let method = AsyncDataGetter::new(syntax.clone(), temp.clone()).await;

                            let returning = match &returning {
                                Some(inner) => Some(Syntax::parse_type(syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                                       resolver.boxed_clone(), inner.clone(), vec!()).await?.finalize(syntax.clone()).await),
                                None => None
                            };

                            if let Ok(found) = check_method(process_manager, method,
                                                            finalized_effects.clone(), syntax, &variables, returning).await {
                                return Ok(Some(found));
                            }
                        }
                    }
                    return Ok(None);
                };

                let mut output = None;
                while output.is_none() && !syntax.lock().unwrap().finished_impls() {
                    output = try_get_impl().await?;
                }

                if output.is_none() {
                    output = try_get_impl().await?;
                }

                output.unwrap()
            } else {
                panic!("Screwed up trait! {} for {:?}", traits, resolver.imports());
            }
        }
        Effects::MethodCall(calling, method, effects, returning) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables, references).await?)
            }

            // Finds methods based off the calling type.
            let method = if let Some(found) = calling {
                let calling = verify_effect(process_manager, resolver.boxed_clone(), *found, syntax, variables, references).await?;
                let return_type = calling.get_return(variables).unwrap();

                // If it's generic, check its trait bounds for the method
                if return_type.name_safe().is_none() {
                    if let Some(mut found) = return_type.find_method(&method) {
                        finalized_effects.insert(0, calling);
                        let mut output = vec!();
                        for (found_trait, function) in &mut found {
                            let temp = AsyncDataGetter { getting: function.clone(), syntax: syntax.clone() }.await;
                            if check_args(&temp, &mut finalized_effects, &syntax, variables).await {
                                output.push((found_trait, temp));
                            }
                        }

                        if output.len() > 1 {
                            return Err(placeholder_error(format!("Duplicate method {} for generic!", method)));
                        } else if output.is_empty() {
                            return Err(placeholder_error(format!("No method {} for generic!", method)));
                        }

                        let (found_trait, found) = output.pop().unwrap();

                        return Ok(FinalizedEffects::GenericMethodCall(found, found_trait.clone(), finalized_effects));
                    }
                }

                // If it's a trait, handle virtual method calls.
                if is_modifier(return_type.inner_struct().data.modifiers, Modifier::Trait) {
                    finalized_effects.insert(0, calling);
                    let method = Syntax::get_function(syntax.clone(), placeholder_error(String::new()),
                                                      format!("{}::{}", return_type.inner_struct().data.name, method), resolver.boxed_clone(), false).await?;
                    let method = AsyncDataGetter::new(syntax.clone(), method).await;

                    if !check_args(&method, &mut finalized_effects, syntax, variables).await {
                        return Err(placeholder_error(format!("Incorrect args to method {}: {:?} vs {:?}", method.data.name,
                                                             method.arguments.iter().map(|field| &field.field.field_type).collect::<Vec<_>>(),
                                                             finalized_effects.iter().map(|effect| effect.get_return(variables).unwrap()).collect::<Vec<_>>())));
                    }

                    let index = return_type.inner_struct().data.functions.iter().position(|found| *found == method.data).unwrap();

                    return Ok(FinalizedEffects::VirtualCall(index, method, finalized_effects));
                }

                finalized_effects.insert(0, calling);
                if let Ok(value) = Syntax::get_function(syntax.clone(), placeholder_error(String::new()),
                                                        method.clone(), resolver.boxed_clone(), true).await {
                    value
                } else {
                    TraitImplWaiter {
                        syntax: syntax.clone(),
                        resolver: resolver.boxed_clone(),
                        method: method.clone(),
                        return_type: return_type.clone(),
                        error: placeholder_error(format!("Unknown method {}", method)),
                    }.await?
                }
            } else {
                Syntax::get_function(syntax.clone(), placeholder_error(format!("Unknown method {}", method)),
                                     method, resolver.boxed_clone(), true).await?
            };

            let returning = match returning {
                Some(inner) => Some(Syntax::parse_type(syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                       resolver, inner, vec!()).await?.finalize(syntax.clone()).await),
                None => None
            };

            let method = AsyncDataGetter::new(syntax.clone(), method).await;
            check_method(process_manager, method, finalized_effects, syntax, variables, returning).await?
        }
        Effects::CompareJump(effect, first, second) =>
            FinalizedEffects::CompareJump(Box::new(
                verify_effect(process_manager, resolver, *effect, syntax, variables, references).await?),
                                          first, second),
        Effects::CreateStruct(target, effects) => {
            let target = Syntax::parse_type(syntax.clone(), placeholder_error(format!("Test")),
                                            resolver.boxed_clone(), target, vec!())
                .await?.finalize(syntax.clone()).await;
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

                final_effects.push((i, verify_effect(process_manager, resolver.boxed_clone(), effect, syntax, variables, references).await?));
            }

            FinalizedEffects::CreateStruct(Some(Box::new(FinalizedEffects::HeapAllocate(target.clone()))),
                                           target, final_effects)
        }
        Effects::Load(effect, target) => {
            let output = verify_effect(process_manager, resolver, *effect, syntax, variables, references).await?;

            let types = output.get_return(variables).unwrap().inner_struct().clone();
            FinalizedEffects::Load(Box::new(output), target.clone(), types)
        }
        Effects::CreateVariable(name, effect) => {
            let effect = verify_effect(process_manager, resolver, *effect, syntax, variables, references).await?;
            let found;
            if let Some(temp_found) = effect.get_return(variables) {
                found = temp_found;
            } else {
                return Err(placeholder_error("No return type!".to_string()));
            };
            variables.variables.insert(name.clone(), found.clone());
            FinalizedEffects::CreateVariable(name.clone(), Box::new(effect), found)
        }
        Effects::NOP() => panic!("Tried to compile a NOP!"),
        Effects::Jump(jumping) => FinalizedEffects::Jump(jumping),
        Effects::LoadVariable(variable) => FinalizedEffects::LoadVariable(variable),
        Effects::Float(float) => store(FinalizedEffects::Float(float)),
        Effects::Int(int) => store(FinalizedEffects::UInt(int as u64)),
        Effects::UInt(uint) => store(FinalizedEffects::UInt(uint)),
        Effects::Bool(bool) => store(FinalizedEffects::Bool(bool)),
        Effects::String(string) => store(FinalizedEffects::String(string)),
        Effects::Char(char) => store(FinalizedEffects::Char(char)),
        Effects::CreateArray(effects) => {
            let mut output = Vec::new();
            for effect in effects {
                output.push(verify_effect(process_manager, resolver.boxed_clone(), effect,
                                          syntax, variables, references).await?);
            }

            let types = output.get(0).map(|found| found.get_return(variables).unwrap());
            if let Some(found) = &types {
                for checking in &output {
                    let returning = checking.get_return(variables).unwrap();
                    if !returning.of_type(found, syntax.clone()).await {
                        return Err(placeholder_error(format!("{:?} isn't a {:?}!", checking, types)));
                    }
                }
            }

            store(FinalizedEffects::CreateArray(types, output))
        }
    };
    return Ok(output);
}


fn store(effect: FinalizedEffects) -> FinalizedEffects {
    return FinalizedEffects::HeapStore(Box::new(effect));
}

//The CheckerVariableManager here is used for the effects calling the method
pub async fn check_method(process_manager: &TypesChecker, mut method: Arc<CodelessFinalizedFunction>,
                          mut effects: Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                          variables: &SimpleVariableManager,
                          returning: Option<FinalizedTypes>) -> Result<FinalizedEffects, ParsingError> {
    if !method.generics.is_empty() {
        let manager = process_manager.clone();

        method = CodelessFinalizedFunction::degeneric(method, Box::new(manager), &effects, syntax, variables, returning).await?;

        let temp_effect = match method.return_type.as_ref() {
            Some(returning) => FinalizedEffects::MethodCall(Some(Box::new(FinalizedEffects::HeapAllocate(returning.clone()))),
                                                            method.clone(), effects),
            None => FinalizedEffects::MethodCall(None, method.clone(), effects),
        };

        return Ok(temp_effect);
    }

    if !check_args(&method, &mut effects, syntax, variables).await {
        return Err(placeholder_error(format!("Incorrect args to method {}: {:?} vs {:?}", method.data.name,
                                             method.arguments.iter().map(|field| &field.field.field_type).collect::<Vec<_>>(),
                                             effects.iter().map(|effect| effect.get_return(variables).unwrap()).collect::<Vec<_>>())));
    }

    return Ok(match method.return_type.as_ref() {
        Some(returning) => FinalizedEffects::MethodCall(Some(Box::new(FinalizedEffects::HeapAllocate(returning.clone()))),
                                                        method, effects),
        None => FinalizedEffects::MethodCall(None, method, effects)
    });
}

pub fn placeholder_error(message: String) -> ParsingError {
    return ParsingError::new("".to_string(), (0, 0), 0, (0, 0), 0, message);
}

pub async fn check_args(function: &Arc<CodelessFinalizedFunction>, args: &mut Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                        variables: &SimpleVariableManager) -> bool {
    if function.arguments.len() != args.len() {
        return false;
    }

    for i in 0..function.arguments.len() {
        let returning = args.get(i).unwrap().get_return(variables);
        if returning.is_some() {
            let inner = returning.as_ref().unwrap();
            let other = &function.arguments.get(i).unwrap().field.field_type;

            if !inner.of_type(other, syntax.clone()).await {
                return false;
            }

            // Only downcast if an implementation was found. Don't downcast if they're of the same type.
            if !inner.of_type_sync(other, None).0 {
                // Handle downcasting
                let temp = args.remove(i);
                // Assumed to only be one function
                let funcs = Syntax::get_implementation_methods(&syntax.lock().unwrap(),
                                                               &temp.get_return(variables).unwrap(), &other).unwrap();

                // Make sure every function is finished adding
                for func in funcs {
                    AsyncDataGetter::new(syntax.clone(), func).await;
                }

                args.insert(i, FinalizedEffects::Downcast(Box::new(temp), other.clone()));
            }
        } else {
            return false;
        }
    }

    return true;
}

pub fn assign_with_priority(operation: String, found: &Arc<StructData>, mut values: Vec<Effects>,
                            inner_operator: String, inner_data: &Arc<StructData>, mut inner_effects: Vec<Effects>,
                            inner_array: bool) -> (Option<Arc<StructData>>, Vec<Effects>) {
    let op_priority = Attribute::find_attribute("priority", &found.attributes)
        .map(|inner| inner.as_int_attribute().unwrap_or(0)).unwrap_or(0);
    let op_parse_left = Attribute::find_attribute("parse_left", &found.attributes)
        .map(|inner| inner.as_bool_attribute().unwrap_or(false)).unwrap_or(false);
    let lhs_priority = Attribute::find_attribute("priority", &inner_data.attributes)
        .map(|inner| inner.as_int_attribute().unwrap_or(0)).unwrap_or(0);

    return if lhs_priority < op_priority || (!op_parse_left && lhs_priority == op_priority) {
        if inner_array {
            if let Effects::CreateArray(inner) = values.last_mut().unwrap() {
                inner.push(inner_effects.remove(0));
            } else {
                panic!("Assumed op args ended with an array when they didn't!")
            }
        } else {
            values.push(inner_effects.remove(0));
        }
        inner_effects.insert(0, Effects::Operation(operation, values));
        (Some(inner_data.clone()), inner_effects)
    } else {
        values.push(Effects::Operation(inner_operator, inner_effects));
        (Some(found.clone()), values)
    };
}