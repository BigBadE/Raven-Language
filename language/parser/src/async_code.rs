use std::future::Future;
use std::sync::{Arc, Mutex};
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::syntax::Syntax;
use syntax::types::Types;
use crate::imports::ImportManager;
use crate::util::parse_struct_args;

pub async fn async_parse_expression(expression_type: ExpressionType, effect: impl Future<Output=Effects>) -> Expression {
    return Expression::new(expression_type, effect.await);
}

pub async fn async_set(name: String, effect: impl Future<Output=Effects>) -> Effects {
    return Effects::Set(Box::new(Effects::String(name)), Box::new(effect.await));
}

pub async fn async_create_struct(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>, structure: String,
                                 args: Vec<(String, impl Future<Output=Effects>)>) -> Effects {
    let structure = Syntax::get_struct(syntax.clone(), structure, import_manager).await;
    let names: Vec<&String> = structure.fields.iter().map(|field| field.field.name).collect();
    let mut out_args = Vec::new();
    for (arg, effect) in args {
        out_args.push((names.iter().position(|found| found == &arg).unwrap(), effect.await))
    }
    return Effects::CreateStruct(Types::Struct(structure), out_args);
}

/// Used for calling methods without a ".", like if they're statically imported or local
pub async fn async_local_method_call(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                                     calling: impl Future<Output=Effects>,
                                     args: Vec<impl Future<Output=Effects>>) -> Effects {
    if let Effects::Load(_, name) = found.await {
        return async_method_call(syntax, import_manager, None, name, args);
    } else {
        panic!("Somehow called local method with non-load last! Report this!");
    }
}

pub async fn async_method_call(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                               last: Option<impl Future<Output=Effects>>, calling: String,
                               args: Vec<impl Future<Output=Effects>>) -> Effects {
    let function = Syntax::get_function(syntax.clone(), match last {
        Some(last) => format!("{}::{}", last.await.get_return(syntax).await.unwrap(), calling),
        None => calling
    }, import_manager);
    let mut arguments = Vec::new();
    for arg in args {
        arguments.push(arg.await);
    }
    return Effects::MethodCall(function, arguments);
}

pub async fn async_field_load(last: Option<impl Future<Output=Effects>>, name: String) -> Effects {
    if let Some(found) = last {
        return Effects::Load(Some(Box::new(found.await)), name);
    }
    return Effects::Load(None, name);
}

pub async fn async_parse_operator(syntax: &Arc<Mutex<Syntax>>, import_manager: Box<ImportManager>,
                                  name: String, args: Vec<impl Future<Output=Effects>>) -> Effects {
    loop {
        let function = Syntax::get_function(syntax.clone(), name.clone(), import_manager.clone()).await;
        //TODO find operator function.
    }
}