use async_recursion::async_recursion;
use syntax::{ParsingError, ProcessManager, SimpleVariableManager};
use syntax::async_util::AsyncDataGetter;
use syntax::code::{degeneric_header, Effects, ExpressionType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, FinalizedCodeBody};
use syntax::r#struct::VOID;
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;

use crate::check_method_call::{check_method, check_method_call};
use crate::check_operator::check_operator;
use crate::CodeVerifier;

pub async fn verify_code(code_verifier: &mut CodeVerifier<'_>, variables: &mut SimpleVariableManager, code: CodeBody, top: bool)
    -> Result<FinalizedCodeBody, ParsingError> {
    let mut body = Vec::new();
    let mut found_end = false;
    for line in code.expressions {
        match &line.effect {
            Effects::CompareJump(_, _, _) => found_end = true,
            Effects::Jump(_) => found_end = true,
            _ => {}
        }

        body.push(FinalizedExpression::new(line.expression_type,
                                           verify_effect(code_verifier, variables, line.effect).await?));

        if let ExpressionType::Return = line.expression_type {
            if let Some(return_type) = code_verifier.return_type.as_ref() {
                let mut last = body.pop().unwrap();
                let last_type = last.effect.get_return(variables).unwrap();
                // Only downcast types that don't match and aren't generic
                if last_type != *return_type && last_type.name_safe().is_some() {
                    if last_type.of_type(return_type, code_verifier.syntax.clone()).await {
                        ImplWaiter {
                            syntax: code_verifier.syntax.clone(),
                            return_type: last_type.clone(),
                            data: return_type.clone(),
                            error: placeholder_error(format!("You shouldn't see this! Report this!")),
                        }.await?;
                        last = FinalizedExpression::new(ExpressionType::Return,
                                                        FinalizedEffects::Downcast(Box::new(last.effect), return_type.clone()));
                    } else {
                        return Err(placeholder_error(format!("Expected {}, found {}", return_type, last_type)));
                    }
                }
                body.push(last);
            }
            return Ok(FinalizedCodeBody::new(body, code.label.clone(), true));
        }
    }

    if !found_end && !top {
        panic!("Code body with label {} doesn't return or jump!", code.label)
    }

    return Ok(FinalizedCodeBody::new(body, code.label.clone(), false));
}

#[async_recursion]
pub async fn verify_effect(code_verifier: &mut CodeVerifier<'_>, variables: &mut SimpleVariableManager, effect: Effects) -> Result<FinalizedEffects, ParsingError> {
    let output = match effect {
        Effects::Paren(inner) => verify_effect(code_verifier, variables, *inner).await?,
        Effects::CodeBody(body) =>
            FinalizedEffects::CodeBody(verify_code(code_verifier, &mut variables.clone(), body, false).await?),
        Effects::Set(first, second) => {
            FinalizedEffects::Set(Box::new(
                verify_effect(code_verifier, variables, *first).await?),
                                  Box::new(
                                      verify_effect(code_verifier, variables, *second).await?))
        },
        Effects::Operation(_, _) => check_operator(code_verifier, variables, effect).await?,
        Effects::ImplementationCall(calling, traits, method, effects, returning) => {
            let mut finalized_effects = Vec::new();
            for effect in effects {
                finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
            }

            let mut finding_return_type;
            if let Effects::NOP() = *calling {
                finding_return_type = FinalizedTypes::Struct(VOID.clone(), None);
            } else {
                let found = verify_effect(code_verifier, variables, *calling).await?;
                finding_return_type = found.get_return(variables).unwrap();
                finding_return_type.fix_generics(&code_verifier.resolver, &code_verifier.syntax).await?;
                finalized_effects.insert(0, found);
            }

            if let Ok(inner) = Syntax::get_struct(code_verifier.syntax.clone(), ParsingError::empty(),
                                                  traits.clone(), code_verifier.resolver.boxed_clone(), vec!()).await {
                let data = inner.finalize(code_verifier.syntax.clone()).await;
                if finding_return_type.of_type_sync(&data, None).0 {
                    let mut i = 0;
                    for found in &data.inner_struct().data.functions {
                        if found.name == method {
                            return Ok(FinalizedEffects::VirtualCall(i,
                                                                    AsyncDataGetter::new(
                                                                        code_verifier.syntax.clone(), found.clone()).await,
                                                                    finalized_effects));
                        } else if found.name.split("::").last().unwrap() == method {
                            let mut target = finding_return_type.find_method(&method).unwrap();
                            if target.len() > 1 {
                                return Err(placeholder_error(format!("Ambiguous function {}", method)));
                            } else if target.is_empty() {
                                return Err(placeholder_error(format!("Unknown function {}", method)));
                            }
                            let (_, target) = target.pop().unwrap();

                            let return_type = finalized_effects[0].get_return(variables).unwrap().unflatten();
                            if let FinalizedTypes::Generic(_, _) = return_type {
                                return Ok(FinalizedEffects::GenericVirtualCall(i, target,
                                                                               AsyncDataGetter::new(
                                                                                   code_verifier.syntax.clone(), found.clone()).await,
                                                                               finalized_effects));
                            }

                            code_verifier.syntax.lock().unwrap().process_manager.handle().lock().unwrap().spawn(target.name.clone(),
                                degeneric_header(target.clone(),
                                                 found.clone(), code_verifier.syntax.clone(), code_verifier.process_manager.cloned(),
                                                 finalized_effects.clone(), variables.clone()));

                            let output = AsyncDataGetter::new(code_verifier.syntax.clone(), target.clone()).await;
                            return Ok(FinalizedEffects::VirtualCall(i,
                                                                    output,
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
                        syntax: code_verifier.syntax.clone(),
                        return_type: finding_return_type.clone(),
                        data: data.clone(),
                        error: placeholder_error(
                            format!("Nothing implements {} for {}", inner, finding_return_type)),
                    }.await?;

                    for temp in &result {
                        if temp.name.split("::").last().unwrap() == method || method.is_empty() {
                            let method = AsyncDataGetter::new(code_verifier.syntax.clone(), temp.clone()).await;

                            let returning = match &returning {
                                Some(inner) => Some(Syntax::parse_type(code_verifier.syntax.clone(), placeholder_error(format!("Bounds error!")),
                                                                       code_verifier.resolver.boxed_clone(), inner.clone(),
                                                                       vec!()).await?.finalize(code_verifier.syntax.clone()).await),
                                None => None
                            };

                            match check_method(&code_verifier.process_manager, method.clone(),
                                                            finalized_effects.clone(), &code_verifier.syntax,
                                                            &variables, &code_verifier.resolver, returning).await {
                                Ok(found) => return Ok(Some(found)),
                                Err(_error) => {}
                            };
                        }
                    }
                    return Ok(None);
                };

                let mut output = None;
                while output.is_none() && !code_verifier.syntax.lock().unwrap().finished_impls() {
                    output = try_get_impl().await?;
                }

                if output.is_none() {
                    output = try_get_impl().await?;
                }

                if output.is_none() {
                    panic!("Failed for {} and {}", finding_return_type, data);
                }
                output.unwrap()
            } else {
                panic!("Screwed up trait! {} for {:?}", traits, code_verifier.resolver.imports());
            }
        }
        Effects::MethodCall(_, _, _, _) => check_method_call(code_verifier, variables, effect).await?,
        Effects::CompareJump(effect, first, second) =>
            FinalizedEffects::CompareJump(Box::new(
                verify_effect(code_verifier, variables, *effect).await?),
                                          first, second),
        Effects::CreateStruct(target, effects) => {
            let target = Syntax::parse_type(code_verifier.syntax.clone(), placeholder_error(format!("Test")),
                                            code_verifier.resolver.boxed_clone(), target, vec!())
                .await?.finalize(code_verifier.syntax.clone()).await;
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

                final_effects.push((i, verify_effect(code_verifier, variables, effect).await?));
            }

            FinalizedEffects::CreateStruct(Some(Box::new(FinalizedEffects::HeapAllocate(target.clone()))),
                                           target, final_effects)
        }
        Effects::Load(effect, target) => {
            let output = verify_effect(code_verifier, variables, *effect).await?;

            let types = output.get_return(variables).unwrap().inner_struct().clone();
            FinalizedEffects::Load(Box::new(output), target.clone(), types)
        }
        Effects::CreateVariable(name, effect) => {
            let effect = verify_effect(code_verifier, variables, *effect).await?;
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
                output.push(verify_effect(code_verifier, variables, effect).await?);
            }

            let types = output.get(0).map(|found| found.get_return(variables).unwrap());
            if let Some(found) = &types {
                for checking in &output {
                    let returning = checking.get_return(variables).unwrap();
                    if !returning.of_type(found, code_verifier.syntax.clone()).await {
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

pub fn placeholder_error(message: String) -> ParsingError {
    return ParsingError::new("".to_string(), (0, 0), 0, (0, 0), 0, message);
}