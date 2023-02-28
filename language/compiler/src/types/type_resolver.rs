use std::collections::HashMap;
use inkwell::values::BasicValueEnum;
use ast::code::Effects;
use ast::function::Arguments;
use ast::type_resolver::TypeResolver;
use crate::compiler::Compiler;

pub struct FunctionTypeResolver<'com, 'ctx> {
    pub compiler: &'com Compiler<'ctx>,
    pub variables: HashMap<String, BasicValueEnum<'ctx>>
}

impl<'com, 'ctx> FunctionTypeResolver<'com, 'ctx> {
    pub fn new(compiler: &'com Compiler<'ctx>) -> Self {
        return Self {
            compiler,
            variables: HashMap::new()
        }
    }

    pub fn get(&self, name: &String) -> Option<&BasicValueEnum<'ctx>> {
        return self.variables.get(name);
    }
}

impl<'com, 'ctx> TypeResolver for FunctionTypeResolver<'com, 'ctx> {
    fn get_method_type(&self, name: &String, _calling: &Option<Effects>, _args: &Arguments) -> Option<String> {
        return self.compiler.functions.get(name).unwrap().0.clone();
    }

    fn get_variable_type(&self, name: &String) -> Option<String> {
        let variable = self.variables.get(name).unwrap().get_type();
        //Reverse lookup the variable name.
        for (name, found_type) in &self.compiler.types.types {
            if variable == *found_type {
                return Some(name.clone());
            }
        }
        return None;
    }
}