use crate::check_code::{placeholder_error, verify_code};
use crate::output::TypesChecker;
use crate::{finalize_generics, CodeVerifier};
use std::sync::Arc;
use std::sync::Mutex;
use syntax::async_util::NameResolver;
use syntax::code::{
    ExpressionType, FinalizedEffects, FinalizedExpression, FinalizedField, FinalizedMemberField,
};
use syntax::function::{
    CodeBody, CodelessFinalizedFunction, FinalizedCodeBody, FinalizedFunction, UnfinalizedFunction,
};
use syntax::syntax::Syntax;
use syntax::types::FinalizedTypes;
use syntax::{is_modifier, Modifier, ParsingError, SimpleVariableManager};

pub async fn verify_function(
    mut function: UnfinalizedFunction,
    syntax: &Arc<Mutex<Syntax>>,
    include_refs: bool,
) -> Result<(CodelessFinalizedFunction, CodeBody), ParsingError> {
    let mut fields = Vec::default();
    for argument in &mut function.fields {
        let field = argument.await?;
        let mut field = FinalizedMemberField {
            modifiers: field.modifiers,
            attributes: field.attributes,
            field: FinalizedField {
                field_type: field.field.field_type.finalize(syntax.clone()).await,
                name: field.field.name,
            },
        };
        if include_refs {
            field.field.field_type = FinalizedTypes::Reference(Box::new(field.field.field_type));
        }

        fields.push(field);
    }

    let return_type = if let Some(return_type) = function.return_type.as_mut() {
        Some(return_type.await?.finalize(syntax.clone()).await)
    } else {
        None
    };

    let codeless = CodelessFinalizedFunction {
        generics: finalize_generics(syntax, function.generics).await?,
        arguments: fields,
        return_type,
        data: function.data.clone(),
    };

    return Ok((codeless, function.code));
}

pub async fn verify_function_code(
    process_manager: &TypesChecker,
    resolver: Box<dyn NameResolver>,
    code: CodeBody,
    codeless: CodelessFinalizedFunction,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<FinalizedFunction, ParsingError> {
    {
        let mut locked = syntax.lock().unwrap();
        if let Some(wakers) = locked.functions.wakers.remove(&codeless.data.name) {
            for waker in wakers {
                waker.wake();
            }
        }

        locked
            .functions
            .data
            .insert(codeless.data.clone(), Arc::new(codeless.clone()));
    }

    //Internal/external/trait functions verify everything but the code.
    if is_modifier(codeless.data.modifiers, Modifier::Internal)
        || is_modifier(codeless.data.modifiers, Modifier::Extern)
    {
        return Ok(codeless.clone().add_code(FinalizedCodeBody::new(
            Vec::default(),
            String::default(),
            true,
        )));
    }

    let mut variable_manager = SimpleVariableManager::for_function(&codeless);
    let mut code_verifier = CodeVerifier {
        process_manager,
        resolver,
        return_type: codeless.return_type.clone(),
        syntax: syntax.clone(),
    };
    let mut code = verify_code(&mut code_verifier, &mut variable_manager, code, true).await?;

    if !code.returns {
        if codeless.return_type.is_none() {
            code.expressions.push(FinalizedExpression::new(
                ExpressionType::Return,
                FinalizedEffects::NOP,
            ));
        } else if !is_modifier(codeless.data.modifiers, Modifier::Trait) {
            return Err(placeholder_error(format!(
                "Function {} returns void instead of a {}!",
                codeless.data.name,
                codeless.return_type.as_ref().unwrap()
            )));
        }
    }

    return Ok(codeless.clone().add_code(code));
}
