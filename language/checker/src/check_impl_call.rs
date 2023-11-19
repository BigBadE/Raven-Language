use syntax::ProcessManager;
use syntax::code::{degeneric_header, Effects, FinalizedEffects};
use syntax::{ParsingError, SimpleVariableManager};
use syntax::async_util::AsyncDataGetter;
use syntax::r#struct::VOID;
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;
use crate::check_code::{placeholder_error, verify_effect};
use crate::check_method_call::check_method;
use crate::CodeVerifier;

pub async fn check_impl_call(code_verifier: &mut CodeVerifier<'_>, variables: &mut SimpleVariableManager, effect: Effects)
                             -> Result<FinalizedEffects, ParsingError> {
    let mut finalized_effects = Vec::new();
    let calling;
    let traits;
    let method;
    let returning;
    if let Effects::ImplementationCall(new_calling, new_traits, new_method,
                                       effects, new_returning) = effect {
        for effect in effects {
            finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
        }
        calling = new_calling;
        traits = new_traits;
        method = new_method;
        returning = new_returning;
    } else {
        unreachable!()
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

                    code_verifier.syntax.lock().unwrap().process_manager.handle().lock().unwrap()
                        .spawn(target.name.clone(), degeneric_header(target.clone(),
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
        return Ok(output.unwrap());
    } else {
        panic!("Screwed up trait! {} for {:?}", traits, code_verifier.resolver.imports());
    }
}