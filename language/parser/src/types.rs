use std::collections::HashMap;
use ast::code::Effects;
use ast::function::Arguments;
use ast::program::Program;
use ast::type_resolver::TypeResolver;

#[derive(Clone)]
pub struct ParsingTypeResolver<'a> {
    program: &'a Program,
    variables: HashMap<String, String>
}

impl<'a> ParsingTypeResolver<'a> {
    pub fn new(program: &'a Program) -> Self {
        return Self {
            program,
            variables: HashMap::new()
        }
    }
}

impl<'a> TypeResolver for ParsingTypeResolver<'a> {
    fn get_method_type(&self, name: &String, _calling: &Option<Effects>, _args: &Arguments) -> Option<String> {
        return match self.program.static_functions.get(name) {
            Some(function) => function.return_type.clone(),
            None => None
        }
    }

    fn get_variable_type(&self, name: &String) -> Option<String> {
        return match self.variables.get(name) {
            Some(found) => Some(found.clone()),
            None => None
        };
    }
}