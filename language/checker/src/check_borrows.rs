use crate::get_return;
use std::collections::HashMap;
use syntax::program::code::{FinalizedEffectType, FinalizedEffects};
use syntax::program::function::{FinalizedCodeBody, FinalizedFunction};
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;
use syntax::SimpleVariableManager;

pub fn check_borrows(function: &FinalizedFunction) {
    check_block_borrows(&function.code);
}

fn check_block_borrows(code: &FinalizedCodeBody) {
    let mut loans = SimpleVariableManager::default();
    for line in code.expressions.iter().rev() {
        check_effect_borrows(&line.effect, &mut loans);
    }
}

fn check_effect_borrows(effect: &FinalizedEffects, variables: &mut SimpleVariableManager) {
    match &effect.types {
        FinalizedEffectType::CreateVariable(variable, value, types) => {}
        FinalizedEffectType::Set(from, to) => {}
        FinalizedEffectType::Load(base, field, types) => {
            let path = trace_path(base);
            let base_type = base.types.get_nongeneric_return(variables).unwrap();
            let variable = variables.variables.entry(path[0].clone()).or_insert(base_type);
        }
        _ => {}
    }
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
