use std::collections::HashMap;
use std::rc::Rc;
use crate::code::Effects;
use crate::function::Function;
use crate::types::Types;

pub trait TypeResolver {
    fn get_type(&self, name: &String) -> Option<Rc<Types>>;

    fn print(&self);

    fn add_operation(&mut self, operation: String, function: String);

    fn get_operations(&self) -> &HashMap<String, String>;

    fn get_function(&self, name: &String) -> Option<&Function>;

    fn add_function(&mut self, name: String, function: Function);

    fn get_method_type(&self, name: &String, calling: &Option<Effects>, args: &Vec<&Effects>) -> Option<Rc<Types>>;

    fn get_variable_type(&self, name: &String) -> Option<Rc<Types>>;

    fn finalize(&mut self);
}