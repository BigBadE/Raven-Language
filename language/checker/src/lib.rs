#![feature(get_mut_unchecked)]

use syntax::async_util::{NameResolver, UnparsedType};

pub mod check_code;
pub mod check_function;
pub mod check_struct;
pub mod output;

static EMPTY: Vec<String> = Vec::new();

pub struct EmptyNameResolver {
    
}

impl NameResolver for EmptyNameResolver {
    fn imports(&self) -> &Vec<String> {
        return &EMPTY;
    }

    fn generic(&self, _name: &String) -> Option<Vec<UnparsedType>> {
        return None;
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(EmptyNameResolver {});
    }
}