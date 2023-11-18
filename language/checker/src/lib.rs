#![feature(get_mut_unchecked, async_closure)]

extern crate core;

use std::sync::Arc;
use std::sync::Mutex;

use indexmap::IndexMap;

use syntax::{ParsingError, ParsingFuture};
use syntax::async_util::NameResolver;
use syntax::syntax::Syntax;
use syntax::types::{FinalizedTypes, Types};

use crate::output::TypesChecker;

pub mod check_function;
pub mod check_code;
pub mod check_method_call;
pub mod check_operator;
pub mod check_struct;
pub mod output;

pub async fn finalize_generics(syntax: &Arc<Mutex<Syntax>>, generics: IndexMap<String, Vec<ParsingFuture<Types>>>)
    -> Result<IndexMap<String, Vec<FinalizedTypes>>, ParsingError> {
    let mut output = IndexMap::new();
    for (generic, value) in generics {
        let mut values = Vec::new();
        for found in value {
            values.push(found.await?.finalize(syntax.clone()).await);
        }
        output.insert(generic, values);
    }
    return Ok(output);
}

pub struct CodeVerifier<'a> {
    process_manager: &'a TypesChecker,
    resolver: Box<dyn NameResolver>,
    return_type: Option<FinalizedTypes>,
    syntax: Arc<Mutex<Syntax>>
}