use std::sync::{Arc, Mutex};

use data::tokens::Span;
use syntax::async_util::AsyncDataGetter;
use syntax::code::{EffectType, Effects, FinalizedEffectType, FinalizedEffects};
use syntax::function::{CodelessFinalizedFunction, FunctionData};
use syntax::syntax::Syntax;
use syntax::top_element_manager::TraitImplWaiter;
use syntax::types::FinalizedTypes;
use syntax::{is_modifier, FinishedTraitImplementor, Modifier, ParsingError, SimpleVariableManager};

use crate::check_code::verify_effect;
use crate::{get_return, CodeVerifier};

/// Checks a method call to make sure it's valid
pub async fn check_method_call(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    let mut finalized_effects = Vec::default();
    let calling;
    let method;
    let returning;
    if let EffectType::MethodCall(new_calling, new_method, effects, new_return_type) = effect.types {
        for effect in effects {
            finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
        }
        calling = new_calling;
        method = new_method;
        returning = new_return_type;
    } else {
        unreachable!()
    }

    let returning = match returning {
        Some((inner, span)) => Some((
            Syntax::parse_type(
                code_verifier.syntax.clone(),
                span.make_error("Bounds error!"),
                code_verifier.resolver.boxed_clone(),
                inner,
                vec![],
            )
            .await?
            .finalize(code_verifier.syntax.clone())
            .await,
            span,
        )),
        None => None,
    };

    // Finds methods based off the calling type.
    let method = if let Some(found) = calling {
        let calling = verify_effect(code_verifier, variables, *found).await?;
        let return_type: FinalizedTypes = get_return(&calling.types, variables, &code_verifier.syntax).await.unwrap();

        // If it's generic, check its trait bounds for the method
        if return_type.name_safe().is_none() {
            if let Some(mut found) = return_type.find_method(&method) {
                let span = calling.span.clone();
                finalized_effects.insert(0, calling);
                let mut output = vec![];
                for (found_trait, function) in &mut found {
                    let temp = AsyncDataGetter { getting: function.clone(), syntax: code_verifier.syntax.clone() }.await;
                    /*
                    TODO figure out how the hell to typecheck this
                    println!("Found {} with {:?}", found_trait.name(), finalized_effects.iter()
                        .map(|inner| inner.get_return(variables).unwrap().to_string()).collect::<Vec<_>>());
                    if check_args(&temp, &resolver, &mut finalized_effects, &syntax, variables).await {*/
                    output.push((found_trait, temp));
                    //}
                }

                if output.len() > 1 {
                    return Err(span.make_error("Duplicate method for generic!"));
                } else if output.is_empty() {
                    return Err(span.make_error("No method for generic!"));
                }

                let (found_trait, found) = output.pop().unwrap();

                return Ok(FinalizedEffects::new(
                    effect.span.clone(),
                    FinalizedEffectType::GenericMethodCall(found, found_trait.clone(), finalized_effects),
                ));
            }
        }

        // If it's a trait, handle virtual method calls.
        if is_modifier(return_type.inner_struct().data.modifiers, Modifier::Trait) {
            finalized_effects.insert(0, calling);

            let method = Syntax::get_function(
                code_verifier.syntax.clone(),
                effect.span.make_error("Failed to find method"),
                format!("{}::{}", return_type.inner_struct().data.name, method),
                code_verifier.resolver.boxed_clone(),
                false,
            )
            .await?;
            let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;

            if !check_args(&method, &mut finalized_effects, &code_verifier.syntax, variables).await {
                return Err(effect.span.make_error("Incorrect args to method"));
            }

            let index = return_type.inner_struct().data.functions.iter().position(|found| *found == method.data).unwrap();

            return Ok(FinalizedEffects::new(
                effect.span.clone(),
                FinalizedEffectType::VirtualCall(index, method, finalized_effects),
            ));
        }

        finalized_effects.insert(0, calling);

        if let Ok(value) = Syntax::get_function(
            code_verifier.syntax.clone(),
            ParsingError::new(Span::default(), "You shouldn't see this! Report this please! Location: Check method call"),
            method.clone(),
            code_verifier.resolver.boxed_clone(),
            true,
        )
        .await
        {
            value
        } else {
            let effects = &finalized_effects;
            let variables = &variables;
            let returning = &returning;
            let return_type = &return_type;
            let process_manager = code_verifier.process_manager;
            let syntax = &code_verifier.syntax;
            let span = &effect.span;
            let checker = async move |implementor: Arc<FinishedTraitImplementor>,
                                      method: Arc<FunctionData>|
                        -> Result<FinalizedEffects, ParsingError> {
                let method = AsyncDataGetter::new(syntax.clone(), method).await;
                let mut process_manager = process_manager.clone();
                implementor
                    .base
                    .resolve_generic(
                        return_type,
                        syntax,
                        &mut process_manager.generics,
                        ParsingError::new(
                            Span::default(),
                            "You shouldn't see this! Report this please! Location: Check method call (inner checker)",
                        ),
                    )
                    .await?;
                check_method(method, effects.clone(), syntax, variables, returning.clone(), span).await
            };

            return TraitImplWaiter {
                syntax: code_verifier.syntax.clone(),
                resolver: code_verifier.resolver.boxed_clone(),
                method: method.clone(),
                return_type: return_type.clone(),
                checker,
                error: effect.span.make_error("Unknown trait method"),
            }
            .await;
        }
    } else {
        if method.contains("::") {
            let possible = method.split("::").collect::<Vec<_>>();
            let structure = possible[possible.len() - 2];

            if let Ok(structure) = Syntax::get_struct(
                code_verifier.syntax.clone(),
                ParsingError::new(
                    Span::default(),
                    "You shouldn't see this! Report this please! Location: Check method call (get struct)",
                ),
                structure.to_string(),
                code_verifier.resolver.boxed_clone(),
                vec![],
            )
            .await
            {
                for implementor in Syntax::get_struct_impl(
                    code_verifier.syntax.clone(),
                    structure.finalize(code_verifier.syntax.clone()).await,
                )
                .await
                {
                    for function in &implementor.functions {
                        if function.name.split("::").last().unwrap() == possible[possible.len() - 1] {
                            let method = AsyncDataGetter::new(code_verifier.syntax.clone(), function.clone()).await;
                            match check_method(
                                method,
                                finalized_effects.clone(),
                                &code_verifier.syntax,
                                variables,
                                returning.clone(),
                                &effect.span,
                            )
                            .await
                            {
                                Ok(result) => return Ok(result),
                                Err(error) => println!("Error: {}", error.message),
                            }
                        }
                    }
                }
            }
        }

        Syntax::get_function(
            code_verifier.syntax.clone(),
            effect.span.make_error("Unknown method"),
            method,
            code_verifier.resolver.boxed_clone(),
            true,
        )
        .await?
    };

    let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;
    return check_method(method, finalized_effects, &code_verifier.syntax, variables, returning, &effect.span).await;
}

/// Checks if a method call is valid
/// The CheckerVariableManager here is used for the effects calling the method
pub async fn check_method(
    method: Arc<CodelessFinalizedFunction>,
    mut effects: Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
    generic_returning: Option<(FinalizedTypes, Span)>,
    span: &Span,
) -> Result<FinalizedEffects, ParsingError> {
    if !check_args(&method, &mut effects, syntax, variables).await {
        return Err(span.make_error("Incorrect args to method!"));
    }

    return Ok(match method.return_type.as_ref() {
        Some(returning) => FinalizedEffects::new(
            span.clone(),
            FinalizedEffectType::MethodCall(
                Some(Box::new(FinalizedEffects::new(Span::default(), FinalizedEffectType::HeapAllocate(returning.clone())))),
                method,
                effects,
                generic_returning.map(|(inner, _)| inner),
            ),
        ),
        None => FinalizedEffects::new(
            span.clone(),
            FinalizedEffectType::MethodCall(None, method, effects, generic_returning.map(|(inner, _)| inner)),
        ),
    });
}

/// Checks to see if arguments are valid
pub async fn check_args(
    function: &Arc<CodelessFinalizedFunction>,
    args: &mut Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
) -> bool {
    if function.arguments.len() != args.len() {
        return false;
    }

    for i in 0..function.arguments.len() {
        let mut arg_return_type = get_return(&args[i].types, variables, syntax).await;
        if !arg_return_type.is_some() {
            return false;
        }
        let arg_return_type = arg_return_type.as_mut().unwrap();
        let base_field_type = &function.arguments[i].field.field_type;

        if !arg_return_type.of_type(base_field_type, syntax.clone()).await {
            return false;
        }

        // Only downcast if an implementation was found and it's not generic. Don't downcast if they're of the same type.
        if !arg_return_type.of_type_sync(base_field_type, None).0 && base_field_type.name_safe().is_some() {
            // Handle downcasting
            let argument = args.remove(i);

            args.insert(
                i,
                FinalizedEffects::new(
                    argument.span.clone(),
                    FinalizedEffectType::Downcast(Box::new(argument), base_field_type.clone(), vec![]),
                ),
            );
        }
    }

    return true;
}
