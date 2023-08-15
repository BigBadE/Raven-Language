use std::{mem, thread};
use std::sync::Arc;
use no_deadlocks::Mutex;
use syntax::code::{Effects, ExpressionType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, display_parenless, FinalizedCodeBody, CodelessFinalizedFunction, FunctionData};
use syntax::{Attribute, ParsingError};
use syntax::syntax::Syntax;
use crate::{CheckerVariableManager, EmptyNameResolver};
use async_recursion::async_recursion;
use syntax::async_util::{AsyncDataGetter, NameResolver};
use syntax::types::FinalizedTypes;
use crate::output::TypesChecker;

pub async fn verify_code(process_manager: &TypesChecker, resolver: &Box<dyn NameResolver>, code: CodeBody, external: bool,
                         syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager, references: bool) -> Result<(bool, FinalizedCodeBody), ParsingError> {
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
            return Ok((true, FinalizedCodeBody::new(body, code.label.clone(), true)));
        }
    }

    return Ok((false, FinalizedCodeBody::new(body, code.label.clone(), false)));
}

//IntelliJ seems to think the operation loop is unreachable for some reason.
#[allow(unreachable_code)]
#[async_recursion]
async fn verify_effect(process_manager: &TypesChecker, resolver: Box<dyn NameResolver>, effect: Effects, external: bool,
                       syntax: &Arc<Mutex<Syntax>>, variables: &mut CheckerVariableManager, references: bool) -> Result<FinalizedEffects, ParsingError> {
    let output = match effect.clone() {
        Effects::CodeBody(body) =>
            FinalizedEffects::CodeBody(verify_code(process_manager, &resolver, body, external,
                                                   syntax, &mut variables.clone(), references).await?.1),
        Effects::Set(first, second) => {
            FinalizedEffects::Set(Box::new(
                verify_effect(process_manager, resolver.boxed_clone(), *first, external, syntax, variables, references).await?),
                                  Box::new(
                                      verify_effect(process_manager, resolver, *second, external, syntax, variables, references).await?))
        }
        Effects::Operation(operation, values) => 'outer: {
            let mut args = Vec::new();
            for arg in values {
                args.push(verify_effect(process_manager, resolver.boxed_clone(), arg, external, syntax, variables, references).await?);
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
                        let operation = AsyncDataGetter::new(syntax.clone(), potential_operation).await;
                        let returning = operation.return_type.as_ref().unwrap().clone();
                        if let Some(new_effect) = check_operation(
                            operation, &args,
                            syntax, Some(Box::new(FinalizedEffects::HeapAllocate(returning))),
                            variables).await? {
                            break 'outer assign_with_priority(new_effect);
                        }
                    }
                }

                Syntax::get_function(syntax.clone(), error.clone(),
                                     operation, true, Box::new(EmptyNameResolver {})).await?;
            }
        }
        Effects::ImplementationCall(calling, traits, method, effects, returning) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(process_manager, resolver.boxed_clone(), effect, external, syntax, variables, references).await?)
            }

            let found = verify_effect(process_manager, resolver.boxed_clone(), *calling, external, syntax, variables, references).await?;
            let return_type = found.get_return(variables).unwrap();
            finalized_effects.push(found);

            if let Ok(inner) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                                  traits, resolver.boxed_clone()).await {
                let mut output = None;
                {
                    let mut result = None;
                    while !syntax.lock().unwrap().async_manager.finished {
                        {
                            let locked = syntax.lock().unwrap();
                            result = locked.get_implementation(
                                &return_type.inner_struct().data,
                                &inner.finalize(syntax.clone()).await.inner_struct().data);
                        }
                        thread::yield_now();
                    }

                    let result = match result {
                        Some(inner) => inner,
                        None => {
                            let locked = syntax.lock().unwrap();
                            match locked.get_implementation(
                                &return_type.inner_struct().data,
                                &inner.finalize(syntax.clone()).await.inner_struct().data) {
                                Some(inner) => inner,
                                None => return Err(
                                    placeholder_error(format!("{} doesn't implement {}", return_type, inner)))
                            }
                        }
                    };

                    for temp in &result {
                        if temp.name == method {
                            output = Some(temp.clone());
                        }
                    }
                }

                let returning = match returning {
                    Some(inner) => Some(Syntax::parse_type(syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                           resolver, inner).await?.finalize(syntax.clone()).await),
                    None => None
                };

                check_method(process_manager, AsyncDataGetter::new(syntax.clone(), output.unwrap()).await,
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
                finalized_effects.push(found);
                if let Ok(value) = Syntax::get_function(syntax.clone(), placeholder_error(String::new()),
                                                        method.clone(), false, resolver.boxed_clone()).await {
                    value
                } else {
                    let output = None;
                    while !syntax.lock().unwrap().async_manager.finished {
                        check(syntax, &resolver, &method, &return_type).await?;
                        thread::yield_now();
                    }
                    if let Some(value) = output {
                        value
                    } else {
                        check(syntax, &resolver, &method, &return_type).await?;
                        if let Some(value) = output {
                            value
                        } else {
                            return Err(placeholder_error(format!("Unknown method {}", method)));
                        }
                    }
                }
            } else {
                Syntax::get_function(syntax.clone(), placeholder_error(format!("Unknown method {}", method)),
                                     method, false, resolver.boxed_clone()).await?
            };

            let returning = match returning {
                Some(inner) => Some(Syntax::parse_type(syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                       resolver, inner).await?.finalize(syntax.clone()).await),
                None => None
            };
            check_method(process_manager, AsyncDataGetter::new(syntax.clone(), method).await,
                         finalized_effects, syntax, variables, returning).await?
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
        Effects::String(string) => store(FinalizedEffects::String(string))
    };
    return Ok(output);
}

async fn check(syntax: &Arc<Mutex<Syntax>>, resolver: &Box<dyn NameResolver>,
               method: &String, return_type: &FinalizedTypes) -> Result<Option<Arc<FunctionData>>, ParsingError> {
    for import in resolver.imports() {
        if let Ok(value) = Syntax::get_struct(syntax.clone(), placeholder_error(String::new()),
                                              import.clone(), resolver.boxed_clone()).await {
            if let Some(value) = syntax.lock().unwrap().get_implementation(
                &return_type.inner_struct().data,
                &value.finalize(syntax.clone()).await.inner_struct().data) {
                for temp in &value {
                    if &temp.name == method {
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

async fn check_method(process_manager: &TypesChecker, mut method: Arc<CodelessFinalizedFunction>,
                      effects: Vec<FinalizedEffects>, syntax: &Arc<Mutex<Syntax>>,
                      variables: &mut CheckerVariableManager,
                      returning: Option<FinalizedTypes>) -> Result<FinalizedEffects, ParsingError> {
    if !method.generics.is_empty() {
        println!("Returning for {}? {} ({:?})", method.data.name, returning.is_some(), method.generics);
        let mut manager = process_manager.clone();

        if let Some(inner) = method.return_type.clone() {
            if let Some(returning) = returning {
                if let Some(old) = inner.resolve_generic(&returning, syntax, placeholder_error("Invalid bounds!".to_string())).await? {
                    if let FinalizedTypes::Generic(name, _) = old {
                        manager.generics.insert(name, returning);
                    } else {
                        panic!("Guh?");
                    }
                }
            }
        }

        println!("Generics: {:?}", method.generics);
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

        println!("Generics: {:?}", method.generics);
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
            println!("{} != {}", returning.as_ref().unwrap(), function.fields.get(i).unwrap().field.field_type);
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