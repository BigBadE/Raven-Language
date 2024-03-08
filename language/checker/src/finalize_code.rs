use syntax::program::code::{FinalizedEffects, FinalizedEffectType};
use syntax::program::function::FinalizedFunction;
use syntax::SimpleVariableManager;

pub fn finalize_code(function: &mut FinalizedFunction) {
    let mut variables = SimpleVariableManager::for_final_function(function);
    for line in &mut function.code.expressions {
        finalize_effect(&mut line.effect, &mut variables);
    }
}

pub fn finalize_effect(effect: &mut FinalizedEffects, variables: &mut SimpleVariableManager) {
    match &mut effect.types {
        FinalizedEffectType::CoroutineYield(inner, return_types) {
            finalize_effect(inner, variables);

        }
        FinalizedEffectType::CreateVariable(name, value, types) => {
            *types = value.types.get_nongeneric_return(variables).unwrap();
            variables.variables.insert(name.clone(), types.clone());
            finalize_effect(value, variables);
        }
        FinalizedEffectType::CompareJump(effect, _, _)
        | FinalizedEffectType::Load(effect, _, _)
        | FinalizedEffectType::Downcast(effect, _, _)
        | FinalizedEffectType::HeapStore(effect)
        | FinalizedEffectType::StackStore(effect) => {
            finalize_effect(effect, variables);
        }
        FinalizedEffectType::CodeBody(body) => {
            for expression in &mut body.expressions {
                finalize_effect(&mut expression.effect, variables);
            }
        }
        FinalizedEffectType::MethodCall(calling, _, arguments, _) => {
            if let Some(found) = calling {
                finalize_effect(found, variables);
            }

            for argument in &mut *arguments {
                finalize_effect(argument, variables);
            }
        }
        FinalizedEffectType::GenericMethodCall(_, _, arguments)
        | FinalizedEffectType::VirtualCall(_, _, arguments, _)
        | FinalizedEffectType::GenericVirtualCall(_, _, _, arguments, _)
        | FinalizedEffectType::CreateArray(_, arguments) => {
            for argument in &mut *arguments {
                finalize_effect(argument, variables);
            }
        }
        FinalizedEffectType::Set(base, value) => {
            finalize_effect(base, variables);
            finalize_effect(value, variables);
        }
        FinalizedEffectType::CreateStruct(storing, _, effects) => {
            if let Some(found) = storing {
                finalize_effect(found, variables);
            }
            for (_, argument) in &mut *effects {
                finalize_effect(argument, variables);
            }
        }
        _ => {}
    }
}
