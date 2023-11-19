use crate::check_code::{placeholder_error, verify_effect};
use crate::output::TypesChecker;
use crate::CodeVerifier;
use std::sync::{Arc, Mutex};
use syntax::async_util::{AsyncDataGetter, NameResolver};
use syntax::code::{Effects, FinalizedEffects};
use syntax::function::CodelessFinalizedFunction;
use syntax::syntax::Syntax;
use syntax::top_element_manager::{ImplWaiter, TraitImplWaiter};
use syntax::types::FinalizedTypes;
use syntax::{is_modifier, Modifier, ParsingError, SimpleVariableManager};

pub async fn check_method_call(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    let mut finalized_effects = Vec::default();
    let calling;
    let method;
    let returning;
    if let Effects::MethodCall(new_calling, new_method, effects, new_return_type) = effect {
        for effect in effects {
            finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
        }
        calling = new_calling;
        method = new_method;
        returning = new_return_type;
    } else {
        unreachable!()
    }

    // Finds methods based off the calling type.
    let method = if let Some(found) = calling {
        let calling = verify_effect(code_verifier, variables, *found).await?;
        let return_type = calling.get_return(variables).unwrap();

        // If it's generic, check its trait bounds for the method
        if return_type.name_safe().is_none() {
            if let Some(mut found) = return_type.find_method(&method) {
                finalized_effects.insert(0, calling);
                let mut output = vec![];
                for (found_trait, function) in &mut found {
                    let temp = AsyncDataGetter {
                        getting: function.clone(),
                        syntax: code_verifier.syntax.clone(),
                    }
                    .await;
                    /*
                    TODO figure out how the hell to typecheck this
                    println!("Found {} with {:?}", found_trait.name(), finalized_effects.iter()
                        .map(|inner| inner.get_return(variables).unwrap().to_string()).collect::<Vec<_>>());
                    if check_args(&temp, &resolver, &mut finalized_effects, &syntax, variables).await {*/
                    output.push((found_trait, temp));
                    //}
                }

                if output.len() > 1 {
                    return Err(placeholder_error(format!(
                        "Duplicate method {} for generic!",
                        method
                    )));
                } else if output.is_empty() {
                    return Err(placeholder_error(format!(
                        "No method {} for generic!",
                        method
                    )));
                }

                let (found_trait, found) = output.pop().unwrap();

                return Ok(FinalizedEffects::GenericMethodCall(
                    found,
                    found_trait.clone(),
                    finalized_effects,
                ));
            }
        }

        // If it's a trait, handle virtual method calls.
        if is_modifier(return_type.inner_struct().data.modifiers, Modifier::Trait) {
            finalized_effects.insert(0, calling);

            let method = Syntax::get_function(
                code_verifier.syntax.clone(),
                placeholder_error(format!(
                    "Failed to find method {}::{}",
                    return_type.inner_struct().data.name,
                    method
                )),
                format!("{}::{}", return_type.inner_struct().data.name, method),
                code_verifier.resolver.boxed_clone(),
                false,
            )
            .await?;
            let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;

            if !check_args(
                &method,
                &*code_verifier.resolver,
                &mut finalized_effects,
                &code_verifier.syntax,
                variables,
            )
            .await
            {
                return Err(placeholder_error(format!(
                    "Incorrect args to method {}: {:?} vs {:?}",
                    method.data.name,
                    method
                        .arguments
                        .iter()
                        .map(|field| &field.field.field_type)
                        .collect::<Vec<_>>(),
                    finalized_effects
                        .iter()
                        .map(|effect| effect.get_return(variables).unwrap())
                        .collect::<Vec<_>>()
                )));
            }

            let index = return_type
                .inner_struct()
                .data
                .functions
                .iter()
                .position(|found| *found == method.data)
                .unwrap();

            return Ok(FinalizedEffects::VirtualCall(
                index,
                method,
                finalized_effects,
            ));
        }

        finalized_effects.insert(0, calling);
        if let Ok(value) = Syntax::get_function(
            code_verifier.syntax.clone(),
            placeholder_error(String::default()),
            method.clone(),
            code_verifier.resolver.boxed_clone(),
            true,
        )
        .await
        {
            value
        } else {
            let returning = match returning {
                Some(inner) => Some(
                    Syntax::parse_type(
                        code_verifier.syntax.clone(),
                        placeholder_error(format!("Bounds error!")),
                        code_verifier.resolver.boxed_clone(),
                        inner,
                        vec![],
                    )
                    .await?
                    .finalize(code_verifier.syntax.clone())
                    .await,
                ),
                None => None,
            };

            let effects = &finalized_effects;
            let variables = &variables;
            let resolver_ref = &*code_verifier.resolver;
            let returning = &returning;
            let process_manager = code_verifier.process_manager;
            let syntax = &code_verifier.syntax;
            let checker = async move |method| -> Result<FinalizedEffects, ParsingError> {
                check_method(
                    process_manager,
                    AsyncDataGetter::new(syntax.clone(), method).await,
                    effects.clone(),
                    syntax,
                    variables,
                    resolver_ref,
                    returning.clone(),
                )
                .await
            };
            return TraitImplWaiter {
                syntax: code_verifier.syntax.clone(),
                resolver: code_verifier.resolver.boxed_clone(),
                method: method.clone(),
                return_type: return_type.clone(),
                checker,
                error: placeholder_error(format!("Unknown method {}", method)),
            }
            .await;
        }
    } else {
        Syntax::get_function(
            code_verifier.syntax.clone(),
            placeholder_error(format!("Unknown method {}", method)),
            method,
            code_verifier.resolver.boxed_clone(),
            true,
        )
        .await?
    };

    let returning = match returning {
        Some(inner) => Some(
            Syntax::parse_type(
                code_verifier.syntax.clone(),
                placeholder_error(format!("Bounds error!")),
                code_verifier.resolver.boxed_clone(),
                inner,
                vec![],
            )
            .await?
            .finalize(code_verifier.syntax.clone())
            .await,
        ),
        None => None,
    };

    let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;
    return check_method(
        &code_verifier.process_manager,
        method,
        finalized_effects,
        &code_verifier.syntax,
        variables,
        &*code_verifier.resolver,
        returning,
    )
    .await;
}

//The CheckerVariableManager here is used for the effects calling the method
pub async fn check_method(
    process_manager: &TypesChecker,
    mut method: Arc<CodelessFinalizedFunction>,
    mut effects: Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
    resolver: &dyn NameResolver,
    returning: Option<FinalizedTypes>,
) -> Result<FinalizedEffects, ParsingError> {
    if !method.generics.is_empty() {
        let manager = process_manager.clone();

        method = CodelessFinalizedFunction::degeneric(
            method,
            Box::new(manager),
            &effects,
            syntax,
            variables,
            resolver,
            returning,
        )
        .await?;

        let temp_effect = match method.return_type.as_ref() {
            Some(returning) => FinalizedEffects::MethodCall(
                Some(Box::new(FinalizedEffects::HeapAllocate(returning.clone()))),
                method.clone(),
                effects,
            ),
            None => FinalizedEffects::MethodCall(None, method.clone(), effects),
        };

        return Ok(temp_effect);
    }

    if !check_args(&method, resolver, &mut effects, syntax, variables).await {
        return Err(placeholder_error(format!(
            "Incorrect args to method {}: {:?} vs {:?}",
            method.data.name,
            method
                .arguments
                .iter()
                .map(|field| &field.field.field_type)
                .collect::<Vec<_>>(),
            effects
                .iter()
                .map(|effect| effect.get_return(variables).unwrap())
                .collect::<Vec<_>>()
        )));
    }

    return Ok(match method.return_type.as_ref() {
        Some(returning) => FinalizedEffects::MethodCall(
            Some(Box::new(FinalizedEffects::HeapAllocate(returning.clone()))),
            method,
            effects,
        ),
        None => FinalizedEffects::MethodCall(None, method, effects),
    });
}

pub async fn check_args(
    function: &Arc<CodelessFinalizedFunction>,
    resolver: &dyn NameResolver,
    args: &mut Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
) -> bool {
    if function.arguments.len() != args.len() {
        return false;
    }

    for i in 0..function.arguments.len() {
        let mut returning = args.get(i).unwrap().get_return(variables);
        if returning.is_some() {
            let inner = returning.as_mut().unwrap();
            let other = &function.arguments.get(i).unwrap().field.field_type;

            inner.fix_generics(resolver, syntax).await.unwrap();
            if !inner.of_type(other, syntax.clone()).await {
                return false;
            }

            // Only downcast if an implementation was found. Don't downcast if they're of the same type.
            if !inner.of_type_sync(other, None).0 {
                // Handle downcasting
                let temp = args.remove(i);
                let return_type = temp.get_return(variables).unwrap();
                // Assumed to only be one function
                let funcs = ImplWaiter {
                    syntax: syntax.clone(),
                    return_type,
                    data: other.clone(),
                    error: placeholder_error(format!("Failed to find impl! Report this!")),
                }
                .await
                .unwrap();

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
