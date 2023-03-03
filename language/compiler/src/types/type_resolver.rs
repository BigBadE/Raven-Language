use std::collections::HashMap;
use inkwell::values::BasicValueEnum;
use ast::code::Effects;
use ast::function::Arguments;
use ast::r#struct::TypeMembers;
use ast::type_resolver::TypeResolver;
use ast::types::Types;
use crate::compiler::Compiler;

#[derive(Clone)]
pub struct CompilerTypeResolver<'ctx> {
    pub compiler: &'ctx Compiler<'ctx>,
    pub variables: HashMap<String, BasicValueEnum<'ctx>>
}

impl<'ctx> CompilerTypeResolver<'ctx> {
    pub fn new(compiler: &'ctx Compiler<'ctx>) -> Self {
        return Self {
            compiler,
            variables: HashMap::new()
        }
    }

    pub fn get(&self, name: &String) -> Option<&BasicValueEnum<'ctx>> {
        return self.variables.get(name);
    }
}

impl<'ctx> TypeResolver<'ctx> for CompilerTypeResolver<'ctx> {
    fn get_type(&self, name: &String) -> Option<&'ctx Types<'ctx>> {
        return self.compiler.types.types.get(name);
    }

    fn add_type(&mut self, name: String, types: Types<'ctx>) {
        //self.compiler.types.types.insert(name, types);
        todo!()
    }

    fn get_types(&self) -> &HashMap<String, Types> {
        return &self.compiler.types.types;
    }

    fn get_method_type(&self, name: &String, _calling: &Option<Effects>, _args: &Arguments) -> Option<&'ctx Types<'ctx>> {
        return self.compiler.functions.get(name).unwrap().0.clone();
    }

    fn get_variable_type(&self, name: &String) -> Option<&'ctx Types<'ctx>> {
        let variable = self.variables.get(name).unwrap().get_type();
        //Reverse lookup the variable name.
        for (found_type, llvm_type) in &self.compiler.types.llvm_types {
            if variable == *llvm_type {
                return Some(found_type);
            }
        }
        return None;
    }
}