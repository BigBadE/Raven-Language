use std::collections::HashMap;
use std::rc::Rc;
use crate::code::Effects;
use crate::function::Function;
use crate::types::{ResolvableTypes, Types};

pub trait TypeResolver {
    fn add_type(&mut self, types: Rc<Types>);

    fn add_function(&mut self, function: Function);

    fn get_function(&self, name: &String) -> &Function;

    fn add_operation(&mut self, name: String, function: String);
}

pub trait FinalizedTypeResolver {
    fn solidify_generics(&mut self, function: &String, generics: HashMap<String, ResolvableTypes>) -> &Function;

    fn finalize(&mut self, resolving: &mut ResolvableTypes);

    fn finalize_func(&mut self, function: &mut Function);

    fn finalize_code(&mut self, function: &String);

    fn get_generic_struct(&self, name: &String) -> Option<&Rc<Types>>;

    fn get_variable(&self, name: &String) -> Option<ResolvableTypes>;

    fn get_operator(&self, effects: &Vec<Effects>, operator: String) -> Option<&Function>;

    fn get_function(&self, name: &String) -> Option<&Function>;
}