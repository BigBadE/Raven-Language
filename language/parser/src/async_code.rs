use std::future::Future;
use std::sync::{Arc, Mutex};
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::ParsingError;
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::imports::ImportManager;

pub async fn async_parse_expression(expression_type: ExpressionType,
                                    effect: impl Future<Output=Result<Effects, ParsingError>>)
                                    -> Result<Expression, ParsingError> {
    return Ok(Expression::new(expression_type, effect.await?));
}

/// Used to provide an already-found value to the async tasks.
pub async fn async_finished<T>(input: T) -> Result<T, ParsingError> {
    return Ok(input);
}

pub async fn async_set(name: String, effect: impl Future<Output=Result<Effects, ParsingError>>)
                       -> Result<Effects, ParsingError> {
    return Ok(Effects::Set(Box::new(Effects::String(name)), Box::new(effect.await?)));
}

pub async fn async_create_struct(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                                 structure: impl Future<Output=Effects>,
                                 args: Vec<(String, impl Future<Output=Result<Effects, ParsingError>>)>)
                                 -> Result<Effects, ParsingError> {
    let structure = match structure.await {
        Effects::Load(_, name) => name,
        _ => return Err(ParsingError::new((0, 0), (0, 0), "Unexpected curly bracket!".to_string()))
    };
    let structure = Syntax::get_struct(syntax.clone(), structure, import_manager).await?;
    let names: Vec<&String> = structure.fields.iter().map(|field| &field.field.name).collect();
    let mut out_args = Vec::new();
    for (arg, effect) in args {
        out_args.push((names.iter().position(|found| *found == &arg).unwrap(), effect.await?))
    }
    return Ok(Effects::CreateStruct(Types::Struct(structure), out_args));
}

/// Used for calling methods without a ".", like if they're statically imported or local
pub async fn async_local_method_call(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                                     calling: impl Future<Output=Result<Effects, ParsingError>>,
                                     args: Vec<impl Future<Output=Result<Effects, ParsingError>>>) -> Result<Effects, ParsingError> {
    if let Effects::Load(_, name) = calling.await? {
        return async_method_call(syntax, import_manager, None, name, args).await;
    } else {
        panic!("Somehow called local method with non-load last! Report this!");
    }
}

pub async fn async_method_call(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                               last: Option<impl Future<Output=Result<Effects, ParsingError>>>, calling: String,
                               args: Vec<impl Future<Output=Result<Effects, ParsingError>>>) -> Result<Effects, ParsingError> {
    let function = Syntax::get_function(syntax.clone(), match last {
        Some(last) => format!("{}::{}", last.await?.get_return(syntax).await?.unwrap(), calling),
        None => calling
    }, import_manager).await?;
    let mut arguments = Vec::new();
    for arg in args {
        arguments.push(arg.await?);
    }
    return Ok(Effects::MethodCall(function, arguments));
}

pub async fn async_field_load(last: Option<impl Future<Output=Result<Effects, ParsingError>>>, name: String)
                              -> Result<Effects, ParsingError> {
    if let Some(found) = last {
        return Ok(Effects::Load(Some(Box::new(found.await?)), name));
    }
    return Ok(Effects::Load(None, name));
}

pub async fn async_parse_operator(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                                  name: String, args: Vec<impl Future<Output=Result<Effects, ParsingError>>>)
                                  -> Result<Effects, ParsingError> {
    loop {
        //TODO find operator function.
        let function = Syntax::get_function(syntax.clone(), name.clone(), import_manager.clone()).await;
    }
}