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
    return Effects::Set(Effects::String(name), effect.await);
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