use async_recursion::async_recursion;
use syntax::async_util::UnparsedType;
use syntax::code::{Effects, ExpressionType, FinalizedEffects, FinalizedExpression};
use syntax::function::{CodeBody, FinalizedCodeBody};
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;
use syntax::{ParsingError, SimpleVariableManager};

use crate::check_impl_call::check_impl_call;
use crate::check_method_call::check_method_call;
use crate::check_operator::check_operator;
use crate::CodeVerifier;

pub async fn verify_code(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    code: CodeBody,
    top: bool,
) -> Result<FinalizedCodeBody, ParsingError> {
    let mut body = Vec::default();
    let mut found_end = false;
    for line in code.expressions {
        match &line.effect {
            Effects::CompareJump(_, _, _) => found_end = true,
            Effects::Jump(_) => found_end = true,
            _ => {}
        }

        body.push(FinalizedExpression::new(
            line.expression_type,
            verify_effect(code_verifier, variables, line.effect).await?,
        ));

        if check_return_type(line.expression_type, code_verifier, &mut body, variables).await? {
            return Ok(FinalizedCodeBody::new(
                body.clone(),
                code.label.clone(),
                true,
            ));
        }
    }

    if !found_end && !top {
        panic!(
            "Code body with label {} doesn't return or jump!",
            code.label
        )
    }

    return Ok(FinalizedCodeBody::new(body, code.label.clone(), false));
}

async fn check_return_type(
    line: ExpressionType,
    code_verifier: &CodeVerifier<'_>,
    body: &mut Vec<FinalizedExpression>,
    variables: &SimpleVariableManager,
) -> Result<bool, ParsingError> {
    if line != ExpressionType::Return {
        return Ok(false);
    }

    let return_type = match code_verifier.return_type.as_ref() {
        Some(value) => value,
        None => return Ok(false),
    };

    let last = body.pop().unwrap();
    let last_type = last.effect.get_return(variables).unwrap();
    // Only downcast types that don't match and aren't generic
    if last_type == *return_type || !last_type.name_safe().is_some() {
        body.push(last);
        return Ok(true);
    }

    return if last_type
        .of_type(return_type, code_verifier.syntax.clone())
        .await
    {
        ImplWaiter {
            syntax: code_verifier.syntax.clone(),
            return_type: last_type.clone(),
            data: return_type.clone(),
            error: placeholder_error(format!("You shouldn't see this! Report this!")),
        }
        .await?;

        body.push(FinalizedExpression::new(
            ExpressionType::Return,
            FinalizedEffects::Downcast(Box::new(last.effect), return_type.clone()),
        ));
        Ok(true)
    } else {
        Err(placeholder_error(format!(
            "Expected {}, found {}",
            return_type, last_type
        )))
    };
}

#[async_recursion]
// skipcq: RS-R1000
pub async fn verify_effect(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    if let Some(found) = finalize_basic(&effect).await {
        return Ok(found);
    }
    let output = match effect {
        Effects::Paren(inner) => verify_effect(code_verifier, variables, *inner).await?,
        Effects::CodeBody(body) => FinalizedEffects::CodeBody(
            verify_code(code_verifier, &mut variables.clone(), body, false).await?,
        ),
        Effects::Set(first, second) => FinalizedEffects::Set(
            Box::new(verify_effect(code_verifier, variables, *first).await?),
            Box::new(verify_effect(code_verifier, variables, *second).await?),
        ),
        Effects::Operation(_, _) => check_operator(code_verifier, variables, effect).await?,
        Effects::ImplementationCall(_, _, _, _, _) => {
            check_impl_call(code_verifier, variables, effect).await?
        }
        Effects::MethodCall(_, _, _, _) => {
            check_method_call(code_verifier, variables, effect).await?
        }
        Effects::CompareJump(effect, first, second) => FinalizedEffects::CompareJump(
            Box::new(verify_effect(code_verifier, variables, *effect).await?),
            first,
            second,
        ),
        Effects::CreateStruct(target, effects) => {
            verify_create_struct(code_verifier, target, effects, variables).await?
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
        Effects::CreateArray(effects) => {
            let mut output = Vec::default();
            for effect in effects {
                output.push(verify_effect(code_verifier, variables, effect).await?);
            }

            let types = output
                .first()
                .map(|found| found.get_return(variables).unwrap());
            check_type(&types, &output, variables, code_verifier).await?;

            store(FinalizedEffects::CreateArray(types, output))
        }
        _ => unreachable!(),
    };
    return Ok(output);
}

async fn finalize_basic(effects: &Effects) -> Option<FinalizedEffects> {
    return Some(match effects {
        Effects::NOP => panic!("Tried to compile a NOP!"),
        Effects::Jump(jumping) => FinalizedEffects::Jump(jumping.clone()),
        Effects::LoadVariable(variable) => FinalizedEffects::LoadVariable(variable.clone()),
        Effects::Float(float) => store(FinalizedEffects::Float(*float)),
        Effects::Int(int) => store(FinalizedEffects::UInt(*int as u64)),
        Effects::UInt(uint) => store(FinalizedEffects::UInt(*uint)),
        Effects::Bool(bool) => store(FinalizedEffects::Bool(*bool)),
        Effects::String(string) => store(FinalizedEffects::String(string.clone())),
        Effects::Char(char) => store(FinalizedEffects::Char(*char)),
        _ => return None,
    });
}

async fn verify_create_struct(
    code_verifier: &mut CodeVerifier<'_>,
    target: UnparsedType,
    effects: Vec<(String, Effects)>,
    variables: &mut SimpleVariableManager,
) -> Result<FinalizedEffects, ParsingError> {
    let target = Syntax::parse_type(
        code_verifier.syntax.clone(),
        placeholder_error(format!("Test")),
        code_verifier.resolver.boxed_clone(),
        target,
        vec![],
    )
    .await?
    .finalize(code_verifier.syntax.clone())
    .await;
    let mut final_effects = Vec::default();
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

    return Ok(FinalizedEffects::CreateStruct(
        Some(Box::new(FinalizedEffects::HeapAllocate(target.clone()))),
        target,
        final_effects,
    ));
}

async fn check_type(
    types: &Option<FinalizedTypes>,
    output: &Vec<FinalizedEffects>,
    variables: &SimpleVariableManager,
    code_verifier: &CodeVerifier<'_>,
) -> Result<(), ParsingError> {
    if let Some(found) = types {
        for checking in output {
            let returning = checking.get_return(variables).unwrap();
            if !returning.of_type(found, code_verifier.syntax.clone()).await {
                return Err(placeholder_error(format!(
                    "{:?} isn't a {:?}!",
                    checking, types
                )));
            }
        }
    }
    return Ok(());
}

fn store(effect: FinalizedEffects) -> FinalizedEffects {
    return FinalizedEffects::HeapStore(Box::new(effect));
}

pub fn placeholder_error(message: String) -> ParsingError {
    return ParsingError::new("".to_string(), (0, 0), 0, (0, 0), 0, message);
}
