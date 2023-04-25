#![feature(get_mut_unchecked)]

use syntax::async_util::NameResolver;
use syntax::types::Types;

pub mod check_code;
pub mod check_function;
pub mod output;

pub struct EmptyNameResolver {
    
}

impl NameResolver for EmptyNameResolver {
    fn resolve<'a>(&'a self, name: &'a String) -> &'a String {
        return name;
    }

    fn generic(&self, name: &String) -> Option<Types> {
        return None;
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(EmptyNameResolver {});
    }
}