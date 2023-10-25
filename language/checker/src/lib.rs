#![feature(get_mut_unchecked)]

extern crate core;

use std::sync::Arc;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use indexmap::IndexMap;
use syntax::types::{FinalizedTypes, Types};
use syntax::{ParsingError, ParsingFuture};
use syntax::syntax::Syntax;

pub mod check_function;
pub mod check_code;
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

pub trait Add<T, E> {}

pub trait AddAndAssign<T, E> {}

impl<T: Add<E, T>, E> AddAndAssign<T, E> for T {

}