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
    fn finalize(&self, resolving: &mut ResolvableTypes);

    fn start_func(&mut self, func: Vec<ResolvableTypes>);

    fn end_func(&mut self);

    fn set_variable(&mut self, name: String, value: ResolvableTypes);

    fn get_variable(&self, name: &String) -> Option<&ResolvableTypes>;

    fn get_operator(&self, effects: &Vec<Effects>, operator: String) -> Option<&Function>;

    fn get_function(&self, name: &String) -> Option<&Function>;
}