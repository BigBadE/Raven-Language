use std::sync::Arc;

use parking_lot::Mutex;

use data::tokens::Span;
use syntax::async_util::NameResolver;
use syntax::errors::{ErrorSource, ParsingError, ParsingMessage};
use syntax::program::code::{
    ExpressionType, FinalizedEffectType, FinalizedEffects, FinalizedExpression, FinalizedField, FinalizedMemberField,
};
use syntax::program::function::{
    CodeBody, CodelessFinalizedFunction, FinalizedCodeBody, FinalizedFunction, UnfinalizedFunction,
};
use syntax::program::syntax::Syntax;
use syntax::{is_modifier, Modifier, ProcessManager, SimpleVariableManager};

use crate::check_code::verify_code;
use crate::output::TypesChecker;
use crate::{finalize_generics, CodeVerifier};

/// Verifies a function and returns its code, which is verified seperate to prevent deadlocks
pub async fn verify_function(
    mut function: UnfinalizedFunction,
    resolver: &Box<dyn NameResolver>,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<(CodelessFinalizedFunction, CodeBody), ParsingError> {
    let mut fields = Vec::default();
    // Verify arguments
    for argument in &mut function.fields {
        let field = argument.await?;
        let field = FinalizedMemberField {
            modifiers: field.modifiers,
            attributes: field.attributes,
            field: FinalizedField {
                field_type: field.field.field_type.finalize(syntax.clone()).await,
                name: field.field.name,
            },
        };

        fields.push(field);
    }

    // Verify return type
    let return_type = if let Some(return_type) = function.return_type.as_mut() {
        Some(return_type.await?.finalize(syntax.clone()).await)
    } else {
        None
    };

    // Return the codeless finalized function
    let codeless = CodelessFinalizedFunction {
        generics: finalize_generics(syntax, resolver, &function.generics).await?,
        arguments: fields,
        return_type,
        data: function.data.clone(),
        parent: match function.parent {
            Some(found) => Some(found.await?.finalize(syntax.clone()).await),
            None => None,
        },
    };

    return Ok((codeless, function.code));
}

/// Verifies the code of a function
pub async fn verify_function_code(
    process_manager: &TypesChecker,
    resolver: Box<dyn NameResolver>,
    code: CodeBody,
    codeless: CodelessFinalizedFunction,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<FinalizedFunction, ParsingError> {
    {
        let mut locked = syntax.lock();
        locked.functions.add_data(codeless.data.clone(), Arc::new(codeless.clone()));
    }

    //Internal/external/trait functions verify everything but the code.
    if is_modifier(codeless.data.modifiers, Modifier::Internal) || is_modifier(codeless.data.modifiers, Modifier::Extern) {
        return Ok(codeless.clone().add_code(FinalizedCodeBody::new(Vec::default(), String::default(), true)));
    }

    let mut variable_manager = SimpleVariableManager::for_function(&codeless);
    let mut process_manager = process_manager.clone();

    for (name, bounds) in finalize_generics(syntax, &resolver, resolver.generics()).await? {
        process_manager.mut_generics().insert(name.clone(), bounds);
    }

    let mut code_verifier = CodeVerifier {
        process_manager: &process_manager,
        resolver,
        return_type: codeless.return_type.clone(),
        syntax: syntax.clone(),
    };

    let mut code = verify_code(&mut code_verifier, &mut variable_manager, code, true).await?;

    // Checks the return type exists
    if !code.returns {
        if codeless.return_type.is_none() {
            code.expressions.push(FinalizedExpression::new(
                ExpressionType::Return(Span::default()),
                FinalizedEffects::new(Span::default(), FinalizedEffectType::NOP),
            ));
        } else if !is_modifier(codeless.data.modifiers, Modifier::Trait) {
            return Err(codeless.data.span.make_error(ParsingMessage::NoReturn()));
        }
    }

    return Ok(codeless.clone().add_code(code));
}
