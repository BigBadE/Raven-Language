use data::tokens::Span;
use std::mem;
use std::sync::Arc;

use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::operation_util::OperationGetter;
use syntax::program::code::{EffectType, Effects, FinalizedEffects};
use syntax::program::r#struct::StructData;
use syntax::{Attribute, SimpleVariableManager};

use crate::check_code::verify_effect;
use crate::CodeVerifier;

/// Checks if an operator call is valid
pub async fn check_operator(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    let operation;
    let mut values;
    if let EffectType::Operation(new_operation, new_values) = effect.types {
        operation = new_operation;
        values = new_values;
    } else {
        unreachable!()
    }

    let error = effect.span.make_error(ParsingMessage::UnknownOperation(operation.clone()));
    // Check if it's two operations that should be combined, like a list ([])
    let outer_operation = combine_operation(&operation, &mut values, code_verifier, &effect.span).await?;

    let operation = if let Some(found) = outer_operation {
        found
    } else {
        OperationGetter { syntax: code_verifier.syntax.clone(), operation: vec![operation], error }.await?
    };

    if Attribute::find_attribute("operation", &operation.attributes).unwrap().as_string_attribute().unwrap().contains("{+}")
    {
        if !matches!(values.first().unwrap().types, EffectType::CreateArray(_)) {
            let first = values.remove(0);
            let span = first.span.clone();
            let effect = EffectType::CreateArray(vec![first]);
            values.push(Effects::new(span, effect));
        }
    }

    let calling;
    if values.len() > 0 {
        calling = Box::new(values.remove(0));
    } else {
        calling = Box::new(Effects::new(Span::default(), EffectType::NOP));
    }

    return verify_effect(
        code_verifier,
        variables,
        Effects::new(
            effect.span.clone(),
            EffectType::ImplementationCall(calling, operation.name.clone(), String::default(), values, None),
        ),
    )
    .await;
}

/// Checks if two operations can be combined
async fn combine_operation(
    operation: &String,
    values: &mut Vec<Effects>,
    code_verifier: &mut CodeVerifier<'_>,
    span: &Span,
) -> Result<Option<Arc<StructData>>, ParsingError> {
    let error = span.make_error(ParsingMessage::UnknownOperation(operation.clone()));

    if values.len() > 0 {
        let mut reading_array = None;
        let mut last = values.pop().unwrap();
        if let EffectType::CreateArray(mut effects) = last.types {
            if effects.len() > 0 {
                last = effects.pop().unwrap();
                reading_array = Some(effects);
            } else {
                last = Effects::new(span.clone(), EffectType::CreateArray(vec![]));
            }
        }

        if let EffectType::Operation(inner_operation, effects) = last.types {
            if operation.ends_with("{}") && inner_operation.starts_with("{}") {
                let combined = operation[0..operation.len() - 2].to_string() + &inner_operation;
                let new_operation = if operation.starts_with("{}") && inner_operation.ends_with("{}") {
                    let mut output = vec![];
                    for i in 0..combined.len() - operation.len() - 2 {
                        let mut temp = combined.clone();
                        temp.truncate(operation.len() + i);
                        output.push(temp);
                    }
                    output
                } else {
                    vec![combined.clone()]
                };

                let getter = OperationGetter {
                    syntax: code_verifier.syntax.clone(),
                    operation: new_operation.clone(),
                    error: error.clone(),
                };

                if let Ok(found) = getter.await {
                    let new_operation =
                        Attribute::find_attribute("operation", &found.attributes).unwrap().as_string_attribute().unwrap();

                    let mut inner_array = false;
                    if let Some(found) = reading_array {
                        values.push(Effects::new(last.span.clone(), EffectType::CreateArray(found)));
                        inner_array = true;
                    }
                    return if new_operation.len() >= combined.len() {
                        if inner_array {
                            if let EffectType::CreateArray(last) = &mut values.last_mut().unwrap().types {
                                for effect in effects {
                                    last.push(effect);
                                }
                            }
                        } else {
                            for effect in effects {
                                values.push(effect);
                            }
                        }
                        Ok(Some(found))
                    } else {
                        let new_inner = "{}".to_string() + &combined[new_operation.replace("{+}", "{}").len()..];

                        let inner_data = OperationGetter {
                            syntax: code_verifier.syntax.clone(),
                            operation: vec![new_inner.clone()],
                            error: error.clone(),
                        }
                        .await?;

                        Ok(operator_pratt_parsing(
                            new_operation.clone(),
                            &found,
                            values,
                            new_inner,
                            &inner_data,
                            effects,
                            inner_array,
                            span.clone(),
                            last.span.clone(),
                        ))
                    };
                } else {
                    if reading_array.is_none() {
                        let outer_data = OperationGetter {
                            syntax: code_verifier.syntax.clone(),
                            operation: vec![operation.clone()],
                            error: error.clone(),
                        }
                        .await?;
                        let inner_data = OperationGetter {
                            syntax: code_verifier.syntax.clone(),
                            operation: vec![inner_operation.clone()],
                            error: error.clone(),
                        }
                        .await?;

                        return Ok(operator_pratt_parsing(
                            operation.clone(),
                            &outer_data,
                            values,
                            inner_operation,
                            &inner_data,
                            effects,
                            false,
                            span.clone(),
                            last.span.clone(),
                        ));
                    }
                }
            }
            last = Effects::new(last.span.clone(), EffectType::Operation(inner_operation, effects))
        }

        if let Some(mut found) = reading_array {
            if let EffectType::CreateArray(inner) = &mut found.last_mut().unwrap().types {
                inner.push(last);
            } else {
                let effect = found.pop().unwrap();
                found.push(Effects::new(effect.span.clone(), EffectType::CreateArray(vec![effect])))
            }
        } else {
            values.push(last);
        }
    }
    return Ok(None);
}

/// Uses pratt parsing to make sure operator calls follow the priorities assigned by the attributes.
pub fn operator_pratt_parsing(
    operation: String,
    found: &Arc<StructData>,
    values: &mut Vec<Effects>,
    inner_operator: String,
    inner_data: &Arc<StructData>,
    mut inner_effects: Vec<Effects>,
    inner_array: bool,
    token: Span,
    inner_token: Span,
) -> Option<Arc<StructData>> {
    let op_priority = Attribute::find_attribute("priority", &found.attributes)
        .map(|inner| inner.as_int_attribute().unwrap_or(0))
        .unwrap_or(0);
    let op_parse_left = Attribute::find_attribute("parse_left", &found.attributes)
        .map(|inner| inner.as_bool_attribute().unwrap_or(false))
        .unwrap_or(false);
    let lhs_priority = Attribute::find_attribute("priority", &inner_data.attributes)
        .map(|inner| inner.as_int_attribute().unwrap_or(0))
        .unwrap_or(0);

    return if lhs_priority < op_priority || (!op_parse_left && lhs_priority == op_priority) {
        if inner_array {
            if let EffectType::CreateArray(inner) = &mut values.last_mut().unwrap().types {
                inner.push(inner_effects.remove(0));
            } else {
                panic!("Assumed op args ended with an array when they didn't!")
            }
        } else {
            values.push(inner_effects.remove(0));
        }
        let mut temp = vec![];
        mem::swap(&mut temp, values);
        inner_effects.insert(0, Effects::new(token, EffectType::Operation(operation, temp)));
        *values = inner_effects;

        Some(inner_data.clone())
    } else {
        values.push(Effects::new(inner_token, EffectType::Operation(inner_operator, inner_effects)));
        Some(found.clone())
    };
}
