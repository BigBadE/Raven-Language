use crate::code::Effects;
use crate::function::Arguments;

pub trait TypeResolver {
    fn get_method_type(&self, name: &String, calling: &Option<Effects>, args: &Arguments) -> Option<String>;
    fn get_variable_type(&self, name: &String) -> Option<String>;
}