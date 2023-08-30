#![feature(get_mut_unchecked)]

use std::collections::HashMap;
use std::sync::Arc;
use no_deadlocks::Mutex;
use indexmap::IndexMap;
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::types::{FinalizedTypes, Types};
use syntax::{ParsingError, ParsingFuture, VariableManager};
use syntax::code::FinalizedEffects;
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

    fn parent(&self) -> &String {
        panic!("Should not be called after finalizing!")
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

#[derive(Debug, Clone)]
pub struct CheckerVariableManager {
    pub variables: HashMap<String, FinalizedTypes>,
    pub variable_instructions: HashMap<String, FinalizedEffects>,
}

impl VariableManager for CheckerVariableManager {
    fn get_variable(&self, name: &String) -> Option<FinalizedTypes> {
        return self.variables.get(name).map(|inner| inner.clone());
    }

    fn get_const_variable(&self, name: &String) -> Option<FinalizedEffects> {
        return self.variable_instructions.get(name).map(|inner| inner.clone());
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