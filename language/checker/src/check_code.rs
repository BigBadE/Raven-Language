use parking_lot::Mutex;
use std::sync::Arc;

use async_recursion::async_recursion;
use data::tokens::Span;
use syntax::async_util::UnparsedType;
use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::program::code::{
    EffectType, Effects, ExpressionType, FinalizedEffectType, FinalizedEffects, FinalizedExpression,
};
use syntax::program::function::{CodeBody, FinalizedCodeBody};
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;
use syntax::SimpleVariableManager;

use crate::check_impl_call::check_impl_call;
use crate::check_method_call::check_method_call;
use crate::check_operator::check_operator;
use crate::degeneric::degeneric_type_fields;
use crate::{get_return, CodeVerifier};

/// Verifies a block of code, linking all method calls and types, and making sure the code is ready to compile.
pub async fn verify_code(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    code: CodeBody,
    top: bool,
) -> Result<FinalizedCodeBody, ParsingError> {
    let mut body = Vec::default();
    let mut found_end = false;
    for line in code.expressions {
        match &line.effect.types {
            EffectType::CompareJump(_, _, _) => found_end = true,
            EffectType::Jump(_) => found_end = true,
            _ => {}
        }

        body.push(FinalizedExpression::new(
            line.expression_type.clone(),
            verify_effect(code_verifier, variables, line.effect).await?,
        ));

        if check_return_type(line.expression_type, code_verifier, &mut body, variables, &code_verifier.syntax).await? {
            return Ok(FinalizedCodeBody::new(body.clone(), code.label.clone(), true));
        }
    }

    if !found_end && !top {
        panic!("Code body with label {} doesn't return or jump!", code.label)
    }

    return Ok(FinalizedCodeBody::new(body, code.label.clone(), false));
}

/// Checks to make sure the return type matches in the code block.
async fn check_return_type(
    line: ExpressionType,
    code_verifier: &CodeVerifier<'_>,
    body: &mut Vec<FinalizedExpression>,
    variables: &SimpleVariableManager,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<bool, ParsingError> {
    let span = match &line {
        ExpressionType::Return(span) => span.clone(),
        _ => return Ok(false),
    };

    let return_type = match code_verifier.return_type.as_ref() {
        Some(value) => value,
        None => return Ok(false),
    };

    let last_effect = body.pop().unwrap();
    let last_effect_type;
    if let Some(found) = get_return(&last_effect.effect.types, variables, syntax).await {
        last_effect_type = found;
    } else {
        // This is an if/for/while block, skip it
        return Ok(true);
    }

    // Only downcast types that don't match and aren't generic
    if last_effect_type == *return_type || !last_effect_type.name_safe().is_some() {
        body.push(last_effect);
        return Ok(true);
    }

    return if last_effect_type.of_type(return_type, code_verifier.syntax.clone()).await {
        body.push(FinalizedExpression::new(
            line,
            FinalizedEffects::new(
                Span::default(),
                FinalizedEffectType::Downcast(Box::new(last_effect.effect), return_type.clone(), vec![]),
            ),
        ));
        Ok(true)
    } else {
        Err(span.make_error(ParsingMessage::UnexpectedReturnType(last_effect_type, return_type.clone())))
    };
}

/// Verifies a single effect
#[async_recursion(Sync)]
// skipcq: RS-R1000 Match statements have complexity calculated incorrectly
pub async fn verify_effect(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    // Some basic effects are handled in finalize_basic
    if let Some(found) = finalize_basic(&effect).await {
        return Ok(found);
    }

    let output = match effect.types {
        EffectType::Paren(inner) => verify_effect(code_verifier, variables, *inner).await?,
        EffectType::CodeBody(body) => FinalizedEffects::new(
            effect.span.clone(),
            FinalizedEffectType::CodeBody(verify_code(code_verifier, &mut variables.clone(), body, false).await?),
        ),
        EffectType::Set(first, second) => FinalizedEffects::new(
            effect.span.clone(),
            FinalizedEffectType::Set(
                Box::new(verify_effect(code_verifier, variables, *first).await?),
                Box::new(verify_effect(code_verifier, variables, *second).await?),
            ),
        ),
        EffectType::Operation(_, _) => check_operator(code_verifier, variables, effect).await?,
        EffectType::ImplementationCall(_, _, _, _, _) => check_impl_call(code_verifier, variables, effect).await?,
        EffectType::MethodCall(_, _, _, _) => check_method_call(code_verifier, variables, effect).await?,
        EffectType::CompareJump(effect, first, second) => FinalizedEffects::new(
            effect.span.clone(),
            FinalizedEffectType::CompareJump(
                Box::new(verify_effect(code_verifier, variables, *effect).await?),
                first,
                second,
            ),
        ),
        EffectType::CreateStruct(target, effects) => verify_create_struct(code_verifier, target, effects, variables).await?,
        EffectType::Load(inner_effect, target) => {
            let output = verify_effect(code_verifier, variables, *inner_effect).await?;
            let types = get_return(&output.types, variables, &code_verifier.syntax).await.unwrap();

            FinalizedEffects::new(effect.span.clone(), FinalizedEffectType::Load(Box::new(output), target.clone(), types))
        }
        EffectType::CreateVariable(name, inner_effect) => {
            let effect = verify_effect(code_verifier, variables, *inner_effect).await?;
            let found;
            if let Some(temp_found) = get_return(&effect.types, variables, &code_verifier.syntax).await {
                found = temp_found;
            } else {
                return Err(effect.span.make_error(ParsingMessage::UnexpectedVoid()));
            };

            variables.variables.insert(name.clone(), found.clone());
            FinalizedEffects::new(
                effect.span.clone(),
                FinalizedEffectType::CreateVariable(name.clone(), Box::new(effect), found),
            )
        }
        EffectType::CreateArray(effects) => {
            let mut output = Vec::default();
            for effect in effects {
                output.push(verify_effect(code_verifier, variables, effect).await?);
            }

            let types = match output.first() {
                Some(found) => get_return(&found.types, variables, &code_verifier.syntax).await,
                None => None,
            };

            check_type(&types, &output, variables, code_verifier, &effect.span).await?;

            FinalizedEffects::new(effect.span.clone(), store(FinalizedEffectType::CreateArray(types, output)))
        }
        _ => unreachable!(),
    };

    return Ok(output);
}

/// Separately handles a few basic effects to declutter the main function
async fn finalize_basic(effects: &Effects) -> Option<FinalizedEffects> {
    return Some(FinalizedEffects::new(
        effects.span.clone(),
        match &effects.types {
            EffectType::NOP => panic!("Tried to compile a NOP!"),
            EffectType::Jump(jumping) => FinalizedEffectType::Jump(jumping.clone()),
            EffectType::LoadVariable(variable) => FinalizedEffectType::LoadVariable(variable.clone()),
            EffectType::Float(float) => store(FinalizedEffectType::Float(*float)),
            EffectType::Int(int) => store(FinalizedEffectType::UInt(*int as u64)),
            EffectType::UInt(uint) => store(FinalizedEffectType::UInt(*uint)),
            EffectType::Bool(bool) => store(FinalizedEffectType::Bool(*bool)),
            EffectType::String(string) => store(FinalizedEffectType::String(string.clone())),
            EffectType::Char(char) => store(FinalizedEffectType::Char(*char)),
            _ => return None,
        },
    ));
}

/// Verifies a CreateStruct call
async fn verify_create_struct(
    code_verifier: &mut CodeVerifier<'_>,
    target: UnparsedType,
    effects: Vec<(String, Effects)>,
    variables: &mut SimpleVariableManager,
) -> Result<FinalizedEffects, ParsingError> {
    let mut target = Syntax::parse_type(
        code_verifier.syntax.clone(),
        Span::default(),
        code_verifier.resolver.boxed_clone(),
        target,
        vec![],
    )
    .await?
    .finalize(code_verifier.syntax.clone())
    .await;

    let mut generics = code_verifier.process_manager.generics.clone();
    let mut final_effects = vec![];
    let fields = target.get_fields();
    for (field_name, effect) in effects {
        let mut i = 0;
        for field in fields {
            if field.field.name == field_name {
                break;
            }
            i += 1;
        }

        if i == fields.len() {
            return Err(effect.span.make_error(ParsingMessage::UnknownField(field_name)));
        }

        let error = effect.span.clone();
        let final_effect = verify_effect(code_verifier, variables, effect).await?;
        get_return(&final_effect.types, variables, &code_verifier.syntax)
            .await
            .unwrap()
            .resolve_generic(&fields[i].field.field_type, &code_verifier.syntax, &mut generics, error)
            .await?;
        final_effects.push((i, final_effect));
    }

    degeneric_type_fields(&mut target, &mut generics, &code_verifier.syntax).await;
    return Ok(FinalizedEffects::new(
        Span::default(),
        FinalizedEffectType::CreateStruct(
            Some(Box::new(FinalizedEffects::new(Span::default(), FinalizedEffectType::HeapAllocate(target.clone())))),
            target,
            final_effects,
        ),
    ));
}

/// Checks if two types are the same
async fn check_type(
    types: &Option<FinalizedTypes>,
    output: &Vec<FinalizedEffects>,
    variables: &SimpleVariableManager,
    code_verifier: &CodeVerifier<'_>,
    span: &Span,
) -> Result<(), ParsingError> {
    if let Some(found) = types {
        for checking in output {
            let returning = get_return(&checking.types, variables, &code_verifier.syntax).await.unwrap();
            if !returning.of_type(found, code_verifier.syntax.clone()).await {
                return Err(span.make_error(ParsingMessage::MismatchedTypes(returning, found.clone())));
            }
        }
    }
    return Ok(());
}

/// Shorthand for storing an effect on the heap
fn store(effect: FinalizedEffectType) -> FinalizedEffectType {
    return FinalizedEffectType::HeapStore(Box::new(FinalizedEffects::new(Span::default(), effect)));
}
