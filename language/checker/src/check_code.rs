use async_recursion::async_recursion;
use data::tokens::Span;
use syntax::async_util::UnparsedType;
use syntax::code::{EffectType, Effects, ExpressionType, FinalizedEffectType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, FinalizedCodeBody};
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;
use syntax::{ParsingError, SimpleVariableManager};

use crate::check_impl_call::check_impl_call;
use crate::check_method_call::check_method_call;
use crate::check_operator::check_operator;
use crate::CodeVerifier;

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

        if check_return_type(line.expression_type, code_verifier, &mut body, variables).await? {
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
) -> Result<bool, ParsingError> {
    let span = match &line {
        ExpressionType::Return(span) => span.clone(),
        _ => return Ok(false),
    };

    let return_type = match code_verifier.return_type.as_ref() {
        Some(value) => value,
        None => return Ok(false),
    };

    let last = body.pop().unwrap();
    let last_type;
    if let Some(found) = last.effect.types.get_return(variables) {
        last_type = found;
    } else {
        // This is an if/for/while block, skip it
        return Ok(true);
    }

    // Only downcast types that don't match and aren't generic
    if last_type == *return_type || !last_type.name_safe().is_some() {
        body.push(last);
        return Ok(true);
    }

    return if last_type.of_type(return_type, code_verifier.syntax.clone()).await {
        let value = ImplWaiter {
            syntax: code_verifier.syntax.clone(),
            return_type: last_type.clone(),
            data: return_type.clone(),
            error: ParsingError::new(
                Span::default(),
                "You shouldn't see this! Report this please! Location: Return type check",
            ),
        }
        .await?;

        body.push(FinalizedExpression::new(
            line,
            FinalizedEffects::new(
                Span::default(),
                FinalizedEffectType::Downcast(Box::new(last.effect), return_type.clone(), value),
            ),
        ));
        Ok(true)
    } else {
        Err(span.make_error("Incorrect return type!"))
    };
}

/// Verifies a single effect
#[async_recursion]
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

            let types = output.types.get_return(variables).unwrap().inner_struct().clone();
            FinalizedEffects::new(effect.span.clone(), FinalizedEffectType::Load(Box::new(output), target.clone(), types))
        }
        EffectType::CreateVariable(name, inner_effect) => {
            let effect = verify_effect(code_verifier, variables, *inner_effect).await?;
            let found;
            if let Some(temp_found) = effect.types.get_return(variables) {
                found = temp_found;
            } else {
                return Err(effect.span.make_error("No return type!"));
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

            let types = output.first().map(|found| found.types.get_return(variables).unwrap());
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
        ParsingError::new(Span::default(), "You shouldn't see this! Report this please! Location: Verify create struct"),
        code_verifier.resolver.boxed_clone(),
        target,
        vec![],
    )
    .await?
    .finalize(code_verifier.syntax.clone())
    .await;
    let mut generics = code_verifier.process_manager.generics.clone();

    if let Some((base, bounds)) = target.inner_generic_type() {
        let mut i = 0;
        for (name, _) in &base.inner_struct().generics {
            generics.insert(name.clone(), bounds[i].clone());
            i += 1;
        }
    }

    target.degeneric(&generics, &code_verifier.syntax).await;
    let mut final_effects = vec![];
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
            return Err(effect.span.make_error("Unknown field!"));
        }

        final_effects.push((i, verify_effect(code_verifier, variables, effect).await?));
    }

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
            let returning = checking.types.get_return(variables).unwrap();
            if !returning.of_type(found, code_verifier.syntax.clone()).await {
                return Err(span.make_error("Incorrect types!"));
            }
        }
    }
    return Ok(());
}

/// Shorthand for storing an effect on the heap
fn store(effect: FinalizedEffectType) -> FinalizedEffectType {
    return FinalizedEffectType::HeapStore(Box::new(FinalizedEffects::new(Span::default(), effect)));
}
