use crate::check_code::{placeholder_error, verify_effect};
use crate::check_method_call::check_method;
use crate::CodeVerifier;
use std::mem;
use syntax::async_util::{AsyncDataGetter, UnparsedType};
use syntax::code::{degeneric_header, Effects, FinalizedEffects};
use syntax::r#struct::VOID;
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;
use syntax::ProcessManager;
use syntax::{ParsingError, SimpleVariableManager};

pub async fn check_impl_call(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    let mut finalized_effects = Vec::default();
    let calling;
    let traits;
    let method;
    let returning;
    if let Effects::ImplementationCall(
        new_calling,
        new_traits,
        new_method,
        effects,
        new_returning,
    ) = effect
    {
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
    if matches!(*calling, Effects::NOP) {
        finding_return_type = FinalizedTypes::Struct(VOID.clone(), None);
    } else {
        let found = verify_effect(code_verifier, variables, *calling).await?;
        finding_return_type = found.get_return(variables).unwrap();
        finding_return_type
            .fix_generics(&*code_verifier.resolver, &code_verifier.syntax)
            .await?;
        finalized_effects.insert(0, found);
    }

    if let Ok(inner) = Syntax::get_struct(
        code_verifier.syntax.clone(),
        ParsingError::empty(),
        traits.clone(),
        code_verifier.resolver.boxed_clone(),
        vec![],
    )
    .await
    {
        let data = inner.finalize(code_verifier.syntax.clone()).await;

        let mut impl_checker = ImplCheckerData {
            code_verifier,
            data: &data,
            returning: &returning,
            method: &method,
            finding_return_type: &finding_return_type,
            finalized_effects: &mut finalized_effects,
            variables,
        };
        if let Some(found) = check_virtual_type(&mut impl_checker).await? {
            return Ok(found);
        }

        let mut output = None;
        while output.is_none() && !code_verifier.syntax.lock().unwrap().finished_impls() {
            output = try_get_impl(&impl_checker).await?;
        }

        if output.is_none() {
            output = try_get_impl(&impl_checker).await?;
        }

        if output.is_none() {
            panic!("Failed for {} and {}", finding_return_type, data);
        }
        return Ok(output.unwrap());
    } else {
        panic!(
            "Screwed up trait! {} for {:?}",
            traits,
            code_verifier.resolver.imports()
        );
    }
}

pub struct ImplCheckerData<'a> {
    code_verifier: &'a CodeVerifier<'a>,
    data: &'a FinalizedTypes,
    returning: &'a Option<UnparsedType>,
    method: &'a String,
    finding_return_type: &'a FinalizedTypes,
    finalized_effects: &'a mut Vec<FinalizedEffects>,
    variables: &'a SimpleVariableManager,
}

async fn check_virtual_type(
    data: &mut ImplCheckerData<'_>,
) -> Result<Option<FinalizedEffects>, ParsingError> {
    if data.finding_return_type.of_type_sync(data.data, None).0 {
        let mut i = 0;
        for found in &data.data.inner_struct().data.functions {
            if found.name == *data.method {
                let mut temp = vec![];
                mem::swap(&mut temp, data.finalized_effects);
                return Ok(Some(FinalizedEffects::VirtualCall(
                    i,
                    AsyncDataGetter::new(data.code_verifier.syntax.clone(), found.clone()).await,
                    temp,
                )));
            } else if found.name.split("::").last().unwrap() == data.method {
                let mut target = data.finding_return_type.find_method(&data.method).unwrap();
                if target.len() > 1 {
                    return Err(placeholder_error(format!(
                        "Ambiguous function {}",
                        data.method
                    )));
                } else if target.is_empty() {
                    return Err(placeholder_error(format!(
                        "Unknown function {}",
                        data.method
                    )));
                }
                let (_, target) = target.pop().unwrap();

                let return_type = data.finalized_effects[0]
                    .get_return(data.variables)
                    .unwrap()
                    .unflatten();
                if matches!(return_type, FinalizedTypes::Generic(_, _)) {
                    let mut temp = vec![];
                    mem::swap(&mut temp, data.finalized_effects);
                    return Ok(Some(FinalizedEffects::GenericVirtualCall(
                        i,
                        target,
                        AsyncDataGetter::new(data.code_verifier.syntax.clone(), found.clone())
                            .await,
                        temp,
                    )));
                }

                data.code_verifier
                    .syntax
                    .lock()
                    .unwrap()
                    .process_manager
                    .handle()
                    .lock()
                    .unwrap()
                    .spawn(
                        target.name.clone(),
                        degeneric_header(
                            target.clone(),
                            found.clone(),
                            data.code_verifier.syntax.clone(),
                            data.code_verifier.process_manager.cloned(),
                            data.finalized_effects.clone(),
                            data.variables.clone(),
                        ),
                    );

                let output =
                    AsyncDataGetter::new(data.code_verifier.syntax.clone(), target.clone()).await;
                let mut temp = vec![];
                mem::swap(&mut temp, data.finalized_effects);
                return Ok(Some(FinalizedEffects::VirtualCall(i, output, temp)));
            }
            i += 1;
        }

        if !data.method.is_empty() {
            return Err(placeholder_error(format!(
                "Unknown method {} in {}",
                data.method, data.data
            )));
        }
    }
    return Ok(None);
}

async fn try_get_impl(
    data: &ImplCheckerData<'_>,
) -> Result<Option<FinalizedEffects>, ParsingError> {
    let result = ImplWaiter {
        syntax: data.code_verifier.syntax.clone(),
        return_type: data.finding_return_type.clone(),
        data: data.data.clone(),
        error: placeholder_error(format!(
            "Nothing implements {} for {}",
            data.data, data.finding_return_type
        )),
    }
    .await?;

    for temp in &result {
        if temp.name.split("::").last().unwrap() == data.method || data.method.is_empty() {
            let method =
                AsyncDataGetter::new(data.code_verifier.syntax.clone(), temp.clone()).await;

            let returning = match &data.returning {
                Some(inner) => Some(
                    Syntax::parse_type(
                        data.code_verifier.syntax.clone(),
                        placeholder_error(format!("Bounds error!")),
                        data.code_verifier.resolver.boxed_clone(),
                        inner.clone(),
                        vec![],
                    )
                    .await?
                    .finalize(data.code_verifier.syntax.clone())
                    .await,
                ),
                None => None,
            };

            match check_method(
                &data.code_verifier.process_manager,
                method.clone(),
                data.finalized_effects.clone(),
                &data.code_verifier.syntax,
                &data.variables,
                &*data.code_verifier.resolver,
                returning,
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
