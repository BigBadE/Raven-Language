use std::collections::HashMap;
use crate::code::Effects;
use crate::function::Arguments;
use crate::types::Types;

pub trait TypeResolver<'a> {
    fn get_type(&self, name: &String) -> Option<&'a Types<'a>>;

    fn add_type(&mut self, name: String, types: Types<'a>);

    fn get_types(&self) -> &HashMap<String, Types>;

    fn get_method_type(&self, name: &String, calling: &Option<Effects>, args: &Arguments) -> Option<&'a Types<'a>>;

    fn get_variable_type(&self, name: &String) -> Option<&'a Types<'a>>;
}