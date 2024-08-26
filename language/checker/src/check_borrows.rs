use parking_lot::Mutex;
use std::sync::Arc;
use syntax::errors::ParsingError;
use syntax::program::code::{FinalizedEffectType, FinalizedEffects};
use syntax::program::function::{FinalizedCodeBody, FinalizedFunction};
use syntax::program::syntax::Syntax;
use syntax::SimpleVariableManager;

pub fn check_borrows(function: &FinalizedFunction, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    return check_block_borrows(&function.code, syntax);
}

fn check_block_borrows(code: &FinalizedCodeBody, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
    let mut loans = SimpleVariableManager::default();
    for line in code.expressions.iter().rev() {
        check_effect_borrows(&line.effect, &mut loans, syntax)?;
    }
    return Ok(());
}

/// The loan checker runs from the bottom to the top with the following rules:
/// 1. If a variable is created, it must not have any loans
/// 2. If a reference is created, it creates a loan to wherever the value is from
/// 3. If a reference is dropped, it removes its loan
fn check_effect_borrows(
    effect: &FinalizedEffects,
    variables: &mut SimpleVariableManager,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<(), ParsingError> {
    match &effect.types {
        FinalizedEffectType::CreateVariable(variable, value, types) => {
            let loans = variables.variables.get(variable).unwrap();
            /*if !loans.is_empty() {
                return Err(ParsingError::new(effect.span, ParsingMessage::IllegalLoan(loans.clone())));
            }*/
        }
        FinalizedEffectType::Load(base, field, types) => {
            let path = trace_path(base);
            let variable = variables.variables.get(&path[0]).unwrap();
        }
        FinalizedEffectType::LoadVariable(variable) => {
            if !variables.variables.contains_key(variable) {
                //variables.variables.insert(variable.clone(), vec![]);
            }
        }
        FinalizedEffectType::FunctionCall(_, function, args, _) | FinalizedEffectType::VirtualCall(_, function, _, args) => {
        }
        _ => {}
    }
    return Ok(());
}

fn try_drop(effect: &FinalizedEffects) {}

/// Traces the loan path for an effect
/// Example:
/// self.data.field
/// trace_path(...) -> [ "self", "data", "field" ]
fn trace_path(effect: &FinalizedEffects) -> Vec<String> {
    match &effect.types {
        FinalizedEffectType::Load(base, field, _) => {
            let mut base = trace_path(base);
            base.push(field.clone());
            return base;
        }
        _ => {}
    }
    return vec![];
}
