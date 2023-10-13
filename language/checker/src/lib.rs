#![feature(get_mut_unchecked)]

extern crate core;

use std::collections::HashMap;
use std::sync::Arc;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use indexmap::IndexMap;
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::types::{FinalizedTypes, Types};
use syntax::{ParsingError, ParsingFuture};
use syntax::syntax::Syntax;

pub mod check_function;
pub mod check_code;
pub mod check_struct;
pub mod output;

static EMPTY: Vec<String> = Vec::new();

pub struct EmptyNameResolver {}

impl NameResolver for EmptyNameResolver {
    fn imports(&self) -> &Vec<String> {
        return &EMPTY;
    }

    fn generic(&self, _name: &String) -> Option<Vec<UnparsedType>> {
        panic!("Should not be called after finalizing!")
    }

    fn generics(&self) -> &HashMap<String, Vec<UnparsedType>> {
        panic!("Should not be called after finalizing!")
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(EmptyNameResolver {});
    }
}

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