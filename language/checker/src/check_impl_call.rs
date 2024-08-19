use std::mem;

use data::tokens::Span;
use syntax::async_util::AsyncDataGetter;
use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::program::code::{EffectType, Effects, FinalizedEffectType, FinalizedEffects};
use syntax::program::r#struct::VOID;
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;
use syntax::top_element_manager::ImplWaiter;
use syntax::SimpleVariableManager;

use crate::check_code::verify_effect;
use crate::check_method_call::check_function;
use crate::degeneric::degeneric_header;
use crate::{get_return, CodeVerifier};

/// Checks an implementation call generated by control_parser or an operator to get the correct method
pub async fn check_impl_call(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    // Get all the ImplementationCall variables
    let mut finalized_effects = Vec::default();
    let calling;
    let traits;
    let method;
    if let EffectType::ImplementationCall(new_calling, new_traits, new_method, effects) = effect.types {
        for effect in effects {
            finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
        }
        calling = new_calling;
        traits = new_traits;
        method = new_method;
    } else {
        unreachable!()
    }

    // Get the return type, or VOID if there is none
    let calling_type;
    let done_calling: Option<Box<FinalizedEffects>>;
    if matches!(calling.types, EffectType::NOP) {
        calling_type = FinalizedTypes::Struct(VOID.clone());
        done_calling = None;
    } else {
        let calling_effect = verify_effect(code_verifier, variables, *calling.clone()).await?;
        calling_type = get_return(&calling_effect.types, variables, &code_verifier.syntax).await.unwrap();
        done_calling = Some(Box::new(calling_effect));
    }

    // Get the trait
    if let Ok(trait_type) = Syntax::get_struct(
        code_verifier.syntax.clone(),
        Span::default(),
        traits.clone(),
        code_verifier.resolver.boxed_clone(),
        vec![],
    )
    .await
    {
        let trait_type = trait_type.finalize(code_verifier.syntax.clone()).await;
        // Simple container for all the data that needs to be stored
        let mut impl_checker = ImplCheckerData {
            calling: done_calling,
            code_verifier,
            trait_type: &trait_type,
            method: &method,
            calling_type: &calling_type,
            finalized_effects: &mut finalized_effects,
            variables,
        };

        // Check if the trait_type matches the calling_type. If so, it's a virtual call (a method call on a trait)
        if let Some(found) = check_virtual_type(&mut impl_checker, &effect.span).await? {
            return Ok(found);
        }

        // If not, wait for an impl to be parsed that fits the criteria
        let mut output = None;
        while output.is_none() && !impl_checker.code_verifier.syntax.lock().finished_impls() {
            // TODO switch this to some kind of pipeline instead of rechecking them all every single time
            output = try_get_impl(&impl_checker, &effect.span).await?;
        }

        // Failed to find an impl
        if output.is_none() {
            output = try_get_impl(&impl_checker, &effect.span).await?;
            if output.is_none() {
                return Err(calling.span.make_error(ParsingMessage::NoTraitImpl(calling_type, trait_type)));
            }
        }

        return Ok(output.unwrap());
    }
    panic!("Screwed up trait! {} for {:?}", traits, code_verifier.resolver.imports());
}

/// All the data used by implementation checkers
pub struct ImplCheckerData<'a> {
    /// The effect being called
    calling: Option<Box<FinalizedEffects>>,
    /// The code verified fields
    code_verifier: &'a CodeVerifier<'a>,
    /// Trait being checked
    trait_type: &'a FinalizedTypes,
    /// The name of the method, can be empty to just return the first found method
    method: &'a String,
    /// The trait to find
    calling_type: &'a FinalizedTypes,
    /// The arguments
    finalized_effects: &'a mut Vec<FinalizedEffects>,
    /// The current variables
    variables: &'a SimpleVariableManager,
}

/// Checks an implementation call to see if it should be a virtual call (a method call on a trait instead of a struct)
async fn check_virtual_type(data: &mut ImplCheckerData<'_>, token: &Span) -> Result<Option<FinalizedEffects>, ParsingError> {
    // If calling_type doesn't extend trait_type, then it's not a virtual call
    if !data.calling_type.of_type_sync(data.trait_type, None).0 {
        return Ok(None);
    }

    let mut i = 0;
    for found in &data.trait_type.inner_struct().data.functions {
        // If the names match, it works
        if found.name == *data.method {
            let mut temp = vec![];
            mem::swap(&mut temp, data.finalized_effects);
            let function = AsyncDataGetter::new(data.code_verifier.syntax.clone(), found.clone()).await;

            return Ok(Some(FinalizedEffects::new(
                token.clone(),
                FinalizedEffectType::VirtualCall(i, function, data.calling.clone().unwrap(), temp),
            )));
        } else if found.name.split("::").last().unwrap() != data.method {
            i += 1;
            continue;
        }

        // Now, try and check the calling type's functions to try and find the method.
        // This assumes that calling_type is a generic type, because that's the only way this can happen.
        let mut target = data.calling_type.find_method(&data.method).unwrap();
        if target.len() > 1 {
            return Err(token.make_error(ParsingMessage::AmbiguousMethod(data.method.clone())));
        } else if target.is_empty() {
            return Err(token.make_error(ParsingMessage::UnknownFunction));
        }
        let (_, target) = target.pop().unwrap();

        // Create a GenericVirtualCall on the generic type
        if data.calling_type.inner_generic_type().is_some() {
            let mut temp = vec![];
            mem::swap(&mut temp, data.finalized_effects);
            return Ok(Some(FinalizedEffects::new(
                token.clone(),
                FinalizedEffectType::GenericVirtualCall(
                    i,
                    target,
                    AsyncDataGetter::new(data.code_verifier.syntax.clone(), found.clone()).await,
                    temp,
                ),
            )));
        }

        data.code_verifier.syntax.lock().process_manager.handle().lock().spawn(
            target.name.clone(),
            degeneric_header(
                target.clone(),
                found.clone(),
                data.code_verifier.syntax.clone(),
                Box::new(data.code_verifier.process_manager.clone()),
                data.finalized_effects.clone(),
                data.variables.clone(),
                token.clone(),
            ),
        );

        let output = AsyncDataGetter::new(data.code_verifier.syntax.clone(), target.clone()).await;
        let mut temp = vec![];
        mem::swap(&mut temp, data.finalized_effects);
        return Ok(Some(FinalizedEffects::new(
            token.clone(),
            FinalizedEffectType::VirtualCall(i, output, data.calling.clone().unwrap(), temp),
        )));
    }

    if !data.method.is_empty() {
        return Err(token.make_error(ParsingMessage::UnknownFunction));
    }
    return Ok(None);
}

/// Tries to get an implementation matching the types passed in
async fn try_get_impl(data: &ImplCheckerData<'_>, span: &Span) -> Result<Option<FinalizedEffects>, ParsingError> {
    let result = ImplWaiter {
        syntax: data.code_verifier.syntax.clone(),
        base_type: data.calling_type.clone(),
        trait_type: data.trait_type.clone(),
        error: span.make_error(ParsingMessage::NoTraitImpl(data.calling_type.clone(), data.trait_type.clone())),
    }
    .await?;

    for temp in result.iter().flat_map(|(_, inner)| inner) {
        if temp.name.split("::").last().unwrap() == data.method || data.method.is_empty() {
            let method = AsyncDataGetter::new(data.code_verifier.syntax.clone(), temp.clone()).await;

            match check_function(
                data.calling.clone(),
                method.clone(),
                data.finalized_effects.clone(),
                &data.code_verifier.syntax,
                &data.variables,
                vec![],
                span,
            )
            .await
            {
                Ok(found) => return Ok(Some(found)),
                Err(_error) => {}
            };
        }
    }
    return Ok(None);
}
