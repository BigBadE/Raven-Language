use parking_lot::Mutex;
use std::sync::Arc;

use data::tokens::Span;
use syntax::async_util::AsyncDataGetter;
use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::program::code::{EffectType, Effects, FinalizedEffectType, FinalizedEffects};
use syntax::program::function::{CodelessFinalizedFunction, FunctionData};
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;
use syntax::top_element_manager::TraitImplWaiter;
use syntax::{is_modifier, FinishedTraitImplementor, Modifier, SimpleVariableManager};

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
    let explicit_generics;
    if let EffectType::MethodCall(new_calling, new_method, effects, new_explicit_generics) = effect.types {
        for effect in effects {
            finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
        }
        calling = new_calling;
        method = new_method;
        explicit_generics = new_explicit_generics;
    } else {
        unreachable!()
    }

    let calling = match calling {
        Some(inner) => Some(verify_effect(code_verifier, variables, *inner).await?),
        None => None,
    };

    let mut final_returning = vec![];
    for value in explicit_generics {
        let span = value.get_span();
        final_returning.push((
            Syntax::parse_type(code_verifier.syntax.clone(), code_verifier.resolver.boxed_clone(), value, vec![])
                .await?
                .finalize(code_verifier.syntax.clone())
                .await,
            span,
        ));
    }
    // Finds methods based off the calling type.
    let method = if let Some(calling) = calling.clone() {
        let return_type: FinalizedTypes = get_return(&calling.types, variables, &code_verifier.syntax).await.unwrap();
        // TODO fix up the errors here
        if final_returning.len() > 0 {
            panic!("Generic bounds added to non-generic type!");
        }

        // If it's generic, check its trait bounds for the method
        if return_type.inner_struct_safe().is_none() {
            // Looking for the method
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
                    return Err(span.make_error(ParsingMessage::AmbiguousMethod(method)));
                } else if output.is_empty() {
                    return Err(span.make_error(ParsingMessage::NoMethod(method, return_type)));
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
            let method = Syntax::get_function(
                code_verifier.syntax.clone(),
                effect.span.clone(),
                format!("{}::{}", return_type.inner_struct().data.name, method),
                code_verifier.resolver.boxed_clone(),
                false,
            )
            .await?;
            let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;

            let calling = Some(Box::new(calling));
            check_args(&method, &calling, &finalized_effects, &code_verifier.syntax, variables, &effect.span).await?;

            let index = return_type.inner_struct().data.functions.iter().position(|found| *found == method.data).unwrap();
            return Ok(FinalizedEffects::new(
                effect.span.clone(),
                FinalizedEffectType::VirtualCall(index, method, calling.unwrap(), finalized_effects),
            ));
        }

        let calling = Some(Box::new(calling));

        // Try to find the function with that name
        if let Ok(value) = Syntax::get_function(
            code_verifier.syntax.clone(),
            Span::default(),
            method.clone(),
            code_verifier.resolver.boxed_clone(),
            true,
        )
        .await
        {
            value
        } else {
            // Used to check if a function is valid
            let checker = async |implementor: Arc<FinishedTraitImplementor>,
                                 method: Arc<FunctionData>|
                   -> Result<FinalizedEffects, ParsingError> {
                let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;
                let mut process_manager = code_verifier.process_manager.clone();
                implementor
                    .base
                    .resolve_generic(&return_type, &code_verifier.syntax, &mut process_manager.generics, Span::default())
                    .await?;
                check_method(
                    calling.clone(),
                    method,
                    finalized_effects.clone(),
                    &code_verifier.syntax,
                    variables,
                    final_returning.clone(),
                    &effect.span,
                )
                .await
            };

            // Try and see if it's a trait method call
            match (TraitImplWaiter {
                syntax: code_verifier.syntax.clone(),
                resolver: code_verifier.resolver.boxed_clone(),
                method: method.clone(),
                return_type: return_type.clone(),
                checker,
                error: ParsingError::new(Span::default(), ParsingMessage::ShouldntSee("Check method call trait waiter")),
            }
            .await)
            {
                Ok(found) => return Ok(found),
                _ => {}
            }

            // If it's not a trait method call, try to find a self-impl method call
            for implementor in Syntax::get_struct_impl(code_verifier.syntax.clone(), return_type.clone()).await {
                for function in &implementor.functions {
                    if function.name.split("::").last().unwrap() == method {
                        let method = AsyncDataGetter::new(code_verifier.syntax.clone(), function.clone()).await;
                        match check_method(
                            calling.clone(),
                            method,
                            finalized_effects.clone(),
                            &code_verifier.syntax,
                            variables,
                            final_returning.clone(),
                            &effect.span,
                        )
                        .await
                        {
                            Ok(result) => return Ok(result),
                            Err(error) => eprintln!("Error: {}", error.message),
                        }
                    }
                }
            }
            return Err(ParsingError::new(effect.span.clone(), ParsingMessage::NoImpl(return_type, method.clone())));
        }
    } else {
        if method.contains("::") {
            let possible = method.split("::").collect::<Vec<_>>();
            let structure = possible[possible.len() - 2];

            if let Ok(structure) = Syntax::get_struct(
                code_verifier.syntax.clone(),
                Span::default(),
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
                                None,
                                method,
                                finalized_effects.clone(),
                                &code_verifier.syntax,
                                variables,
                                final_returning.clone(),
                                &effect.span,
                            )
                            .await
                            {
                                Ok(result) => return Ok(result),
                                Err(error) => eprintln!("Error: {}", error.message),
                            }
                        }
                    }
                }
            }
        }

        Syntax::get_function(
            code_verifier.syntax.clone(),
            effect.span.clone(),
            method,
            code_verifier.resolver.boxed_clone(),
            true,
        )
        .await?
    };

    let method = AsyncDataGetter::new(code_verifier.syntax.clone(), method).await;
    return check_method(
        calling.map(|inner| Box::new(inner)),
        method,
        finalized_effects,
        &code_verifier.syntax,
        variables,
        final_returning,
        &effect.span,
    )
    .await;
}

/// Checks if a method call is valid
/// The CheckerVariableManager here is used for the effects calling the method
pub async fn check_method(
    calling: Option<Box<FinalizedEffects>>,
    method: Arc<CodelessFinalizedFunction>,
    effects: Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
    generic_returning: Vec<(FinalizedTypes, Span)>,
    span: &Span,
) -> Result<FinalizedEffects, ParsingError> {
    check_args(&method, &calling, &effects, syntax, variables, span).await?;

    return Ok(FinalizedEffects::new(
        span.clone(),
        FinalizedEffectType::MethodCall(calling, method, effects, generic_returning),
    ));
}

/// Checks to see if arguments are valid
pub async fn check_args(
    function: &Arc<CodelessFinalizedFunction>,
    calling: &Option<Box<FinalizedEffects>>,
    args: &Vec<FinalizedEffects>,
    syntax: &Arc<Mutex<Syntax>>,
    variables: &SimpleVariableManager,
    span: &Span,
) -> Result<(), ParsingError> {
    let mut length = args.len();
    if calling.is_some() {
        length += 1;
    }
    if function.arguments.len() != length {
        return Err(span.make_error(ParsingMessage::MissingArgument(function.arguments.len() as u64, length as u64)));
    }

    for i in 0..function.arguments.len() {
        let types = if calling.is_some() {
            if i == 0 {
                calling.as_ref().unwrap()
            } else {
                &args[i - 1]
            }
        } else {
            &args[i]
        };
        let mut arg_return_type = get_return(&types.types, variables, syntax).await;
        if !arg_return_type.is_some() {
            return Err(span.make_error(ParsingMessage::UnexpectedVoid()));
        }
        let arg_return_type = arg_return_type.as_mut().unwrap();
        let base_field_type = &function.arguments[i].field.field_type;

        if !arg_return_type.of_type(base_field_type, syntax.clone()).await {
            return Err(span.make_error(ParsingMessage::MismatchedTypes(arg_return_type.clone(), base_field_type.clone())));
        }
    }

    return Ok(());
}
