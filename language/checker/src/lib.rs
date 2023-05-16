#![feature(get_mut_unchecked)]

use syntax::async_util::{NameResolver, UnparsedType};

pub mod check_code;
pub mod check_function;
pub mod check_struct;
pub mod output;

pub struct EmptyNameResolver {
    
}

impl NameResolver for EmptyNameResolver {
    fn resolve<'a>(&'a self, name: &'a String) -> &'a String {
        return name;
    }

    fn generic(&self, _name: &String) -> Option<Vec<UnparsedType>> {
        return None;
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(EmptyNameResolver {});
    }
}