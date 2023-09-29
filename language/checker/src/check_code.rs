use std::{mem, thread};
use std::sync::Arc;
use no_deadlocks::Mutex;
use syntax::code::{Effects, ExpressionType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, FinalizedCodeBody, CodelessFinalizedFunction, FunctionData};
use syntax::{Attribute, CheckerVariableManager, ParsingError};
use syntax::syntax::Syntax;
use async_recursion::async_recursion;
use syntax::async_util::{AsyncDataGetter, NameResolver};
use syntax::operation_util::OperationGetter;
use syntax::r#struct::VOID;
use syntax::types::FinalizedTypes;
use crate::output::TypesChecker;

pub async fn verify_code(process_manager: &TypesChecker, resolver: &Box<dyn NameResolver>, code: CodeBody, external: bool,
                         syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager, references: bool) -> Result<FinalizedCodeBody, ParsingError> {
    let mut body = Vec::new();
    for line in code.expressions {
        body.push(FinalizedExpression::new(line.expression_type,
                                           verify_effect(process_manager, resolver.boxed_clone(),
                                                         line.effect, external, syntax, variables, references).await?));

        if let ExpressionType::Return = line.expression_type {
            if external {
                //Load if the function is external
                let effect = FinalizedEffects::PointerLoad(Box::new(body.pop().unwrap().effect));
                body.push(FinalizedExpression::new(ExpressionType::Return, effect));
            }
            return Ok(FinalizedCodeBody::new(body, code.label.clone(), true));
        }
    }

    return Ok(FinalizedCodeBody::new(body, code.label.clone(), false));
}

//IntelliJ seems to think the operation loop is unreachable for some reason.
#[allow(unreachable_code)]
#[async_recursion]
async fn verify_effect(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, effect: Effects, external: bool,
                       syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager, references: bool) -> Result<FinalizedEffects, ParsingError> {
    let output = match effect {
        Effects::CodeBody(body) =>
            FinalizedEffects::CodeBody(verify_code(process_manager, &resolver, body, external,
                                                   syntax, &mut variables.clone(), references).await?),
        Effects::Set(first, second) => {
            FinalizedEffects::Set(Box::new(
                verify_effect(process_manager, resolver.boxed_clone(), *first, external, syntax, variables, references).await?),
                                  Box::new(
                                      verify_effect(process_manager, resolver, *second, external, syntax, variables, references).await?))
        }
        Effects::Operation(operation, mut values) => {
            let error = ParsingError::new(String::new(), (0, 0), 0,
                                          (0, 0), 0, format!("Failed to find operation {} with {:?}", operation, values));
            let mut outer_operation = None;
            if values.len() > 0 {
                let mut last = values.last().unwrap();
                if let Effects::CreateArray(effects) = last {
                    if effects.len() > 0 {
                        last = effects.last().unwrap();
                    }
                }
                if let Effects::Operation(inner_operation, _) = last {
                    if operation.ends_with("{}") && inner_operation.starts_with("{}") {
                        let getter = OperationGetter {
                            syntax: syntax.clone(),
                            operation: operation[0..operation.len() - 2].to_string() + &inner_operation,
                            error: error.clone(),
                        };
                        if let Ok(found) = getter.await {
                            outer_operation = Some(found);
                        }
                    }
                }
            }

            let operation = if let Some(found) = outer_operation {
                let top = values.pop().unwrap();
                if let Effects::CreateArray(mut inner) = top {
                    if let Effects::Operation(_, found) = inner.pop().unwrap() {
                        for effect in found {
                            inner.push(effect);
                        }
                    }
                    values.push(Effects::CreateArray(inner));
                } else if let Effects::Operation(_, found) = top {
                    for value in found {
                        values.push(value);
                    }
                }
                found
            } else {
                OperationGetter {
                    syntax: syntax.clone(),
                    operation,
                    error,
                }.await?
            };

            let calling;
            if values.len() > 0 {
                calling = Box::new(values.remove(0));
            } else {
                calling = Box::new(Effects::NOP());
            }

            verify_effect(process_manager, resolver,
                          Effects::ImplementationCall(calling, operation.name.clone(),
                                                      String::new(), values, None),
                          external, syntax, variables, references).await?
        }
        Effects::ImplementationCall(calling, traits, method, effects, returning) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, external, syntax, variables, references).await?)
            }

            let return_type;
            if let Effects::NOP() = *calling {
                return_type = FinalizedTypes::Struct(VOID.clone());
            } else {
                let found = verify_effect(process_manager, resolver.boxed_clone(), *calling, external, syntax, variables, references).await?;
                return_type = found.get_return(variables).unwrap();
                finalized_effects.insert(0, found);
            }

            if let Ok(inner) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                                  traits, resolver.boxed_clone()).await {
                let mut output = None;
                {
                    let mut result = None;
                    let data = inner.finalize(syntax.clone()).await;
                    let data = &data.inner_struct().data;
                    while !syntax.lock().unwrap().finished_impls() {
                        {
                            let locked = syntax.lock().unwrap();
                            result = locked.get_implementation(&return_type, data);
                        }
                        thread::yield_now();
                    }

                    let result = match result {
                        Some(inner) => inner,
                        None => {
                            let locked = syntax.lock().unwrap();
                            match locked.get_implementation(&return_type, data) {
                                Some(inner) => inner,
                                None => return Err(
                                    placeholder_error(format!("{} doesn't implement {}", return_type, inner)))
                            }
                        }
                    };

                    for temp in &result {
                        if temp.name == method || method.is_empty() {
                            output = Some(temp.clone());
                        }
                    }
                }

                let output = output.unwrap();
                let method = AsyncDataGetter::new(syntax.clone(), output).await;

                let returning = match returning {
                    Some(inner) => Some(Syntax::parse_type(syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                           resolver, inner).await?.finalize(syntax.clone()).await),
                    None => None
                };
                check_method(process_manager, method,
                             finalized_effects, syntax, variables, returning).await?
            } else {
                panic!("Screwed up trait!");
            }
        }
        Effects::MethodCall(calling, method, effects, returning) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, external, syntax, variables, references).await?)
            }

            let method = if let Some(found) = calling {
                let found = verify_effect(process_manager, resolver.boxed_clone(), *found, external, syntax, variables, references).await?;
                let return_type = found.get_return(variables).unwrap();
                finalized_effects.insert(0, found);
                if let Ok(value) = Syntax::get_function(syntax.clone(), placeholder_error(String::new()),
                                                        method.clone(), resolver.boxed_clone(), true).await {
                    value
                } else {
                    let mut output = None;
                    while output.is_none() && !syntax.lock().unwrap().finished_impls() {
                        output = check(syntax, &resolver, &method, &return_type).await?;
                        thread::yield_now();
                    }

                    if let Some(value) = output {
                        value
                    } else {
                        if let Some(value) = check(syntax, &resolver, &method, &return_type).await? {
                            value
                        } else {
                            return Err(placeholder_error(format!("Unknown method {}", method)));
                        }
                    }
                }
            } else {
                Syntax::get_function(syntax.clone(), placeholder_error(format!("Unknown method {}", method)),
                                     method, resolver.boxed_clone(), true).await?
            };

            let returning = match returning {
                Some(inner) => Some(Syntax::parse_type(syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                       resolver, inner).await?.finalize(syntax.clone()).await),
                None => None
            };

            let method = AsyncDataGetter::new(syntax.clone(), method).await;
            check_method(process_manager, method, finalized_effects, syntax, variables, returning).await?
        }
        Effects::CompareJump(effect, first, second) =>
            FinalizedEffects::CompareJump(Box::new(
                verify_effect(process_manager, resolver, *effect, external, syntax, variables, references).await?),
                                          first, second),
        Effects::CreateStruct(target, effects) => {
            let mut target = Syntax::parse_type(syntax.clone(), placeholder_error(format!("Test")),
                                                resolver.boxed_clone(), target)
                .await?.finalize(syntax.clone()).await;
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

                final_effects.push((i, verify_effect(process_manager, resolver.boxed_clone(), effect, external, syntax, variables, references).await?));
            }

            FinalizedEffects::CreateStruct(Some(Box::new(FinalizedEffects::HeapAllocate(target.clone()))),
                                           target, final_effects)
        }
        Effects::Load(effect, target) => {
            let output = verify_effect(process_manager, resolver, *effect, external, syntax, variables, references).await?;

            let types = output.get_return(variables).unwrap().inner_struct().clone();
            FinalizedEffects::Load(Box::new(output), target.clone(), types)
        }
        Effects::CreateVariable(name, effect) => {
            let effect = verify_effect(process_manager, resolver, *effect, external, syntax, variables, references).await?;
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
        Effects::CreateArray(effects) => {
            let mut output = Vec::new();
            for effect in effects {
                output.push(verify_effect(process_manager, resolver.boxed_clone(), effect,
                                          external, syntax, variables, references).await?);
            }

            let types = output.get(0).map(|found| found.get_return(variables).unwrap());
            if let Some(found) = &types {
                for checking in &output {
                    if !checking.get_return(variables).unwrap().of_type(found, syntax) {
                        return Err(placeholder_error(format!("{:?} isn't a {:?}!", checking, types)));
                    }
                }
            }

            store(FinalizedEffects::CreateArray(types, output))
        }
    };
    return Ok(output);
}

async fn check(syntax: &Arc<Mutex<Syntax>>, resolver: &Box<dyn NameResolver>,
               method: &String, return_type: &FinalizedTypes) -> Result<Option<Arc<FunctionData>>, ParsingError> {
    for import in resolver.imports() {
        if let Ok(value) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                              import.clone(), resolver.boxed_clone()).await {
            let value = value.finalize(syntax.clone()).await;
            if let Some(value) = syntax.lock().unwrap().get_implementation(
                &return_type,
                &value.inner_struct().data) {
                for temp in &value {
                    if &temp.name.split("::").last().unwrap() == method {
                        return Ok(Some(temp.clone()));
                    }
                }
            }
        }
    };
    return Ok(None);
}

fn store(effect: FinalizedEffects) -> FinalizedEffects {
    return FinalizedEffects::HeapStore(Box::new(effect));
}

//The CheckerVariableManager here is used for the effects calling the method
pub async fn check_method(process_manager: &TypesChecker, mut method: Arc<CodelessFinalizedFunction>,
                      effects: Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                      variables: &mut CheckerVariableManager,
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

    if !check_args(&method, &effects, syntax, variables)? {
        return Err(placeholder_error(format!("Incorrect args to method {}: {:?} vs {:?}", method.data.name,
                                             method.fields.iter().map(|field| &field.field.field_type).collect::<Vec<_>>(),
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

pub async fn check_operation(operation: Arc<CodelessFinalizedFunction>, values: &Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                             storing: Option<Box<FinalizedEffects>>, variables: &mut CheckerVariableManager)
                             -> Result<Option<FinalizedEffects>, ParsingError> {
    if check_args(&operation, &values, syntax, variables)? {
        return Ok(Some(FinalizedEffects::MethodCall(storing, operation, values.clone())));
    }
    return Ok(None);
}

pub fn check_args(function: &Arc<CodelessFinalizedFunction>, args: &Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                  variables: &mut CheckerVariableManager) -> Result<bool, ParsingError> {
    if function.fields.len() != args.len() {
        return Ok(false);
    }

    for i in 0..function.fields.len() {
        let returning = args.get(i).unwrap().get_return(variables);
        if returning.is_some() && !returning.as_ref().unwrap().of_type(
            &function.fields.get(i).unwrap().field.field_type, syntax) {
            return Ok(false);
        }
    }

    return Ok(true);
}

pub fn assign_with_priority(operator: FinalizedEffects) -> FinalizedEffects {
    //Needs ownership of the value
    let (target, func, mut effects) = if let FinalizedEffects::MethodCall(target, func, effects) = operator {
        (target, func, effects)
    } else {
        panic!("If your seeing this, something went VERY wrong");
    };
    if effects.len() != 2 {
        return FinalizedEffects::MethodCall(None, func, effects);
    }

    let op_priority = match Attribute::find_attribute("priority", &func.data.attributes) {
        Some(found) => match found {
            Attribute::Integer(_, priority) => *priority,
            _ => 0,
        },
        None => 0
    };

    let op_parse_left = match Attribute::find_attribute("parse_left", &func.data.attributes) {
        Some(found) => match found {
            Attribute::Bool(_, priority) => *priority,
            _ => true,
        },
        None => true
    };

    let lhs = effects.remove(0);

    let lhs_priority = match Attribute::find_attribute("priority", &func.data.attributes) {
        Some(found) => match found {
            Attribute::Integer(_, priority) => *priority,
            _ => 0,
        },
        None => 0
    };

    match lhs {
        // Code explained using the following example: 1 + 2 / 2
        FinalizedEffects::MethodCall(lhs_target, lhs_func, mut lhs) => {
            // temp_lhs = (1 + 2), operator = {} / 2
            if lhs_priority < op_priority || (!op_parse_left && lhs_priority == op_priority) {
                // temp_lhs = 1 + {}, operator = 2 / 2
                mem::swap(lhs.last_mut().unwrap(), effects.first_mut().unwrap());

                // 1 + (2 / 2)
                mem::swap(lhs.last_mut().unwrap(), &mut FinalizedEffects::MethodCall(target, func, effects));

                return FinalizedEffects::MethodCall(lhs_target, lhs_func.clone(), lhs);
            } else {
                effects.insert(0, FinalizedEffects::MethodCall(lhs_target, lhs_func.clone(), lhs));
            }
        }
        _ => effects.insert(0, lhs)
    }

    return FinalizedEffects::MethodCall(target, func, effects);
}