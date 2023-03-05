use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use inkwell::context::Context;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue};
use ast::code::Effects;
use ast::function::Function;
use ast::Modifier;
use ast::r#struct::{Struct, TypeMembers};
use ast::type_resolver::TypeResolver;
use ast::types::Types;

#[derive(Clone)]
pub struct CompilerTypeResolver<'ctx> {
    pub functions: Rc<HashMap<String, (Function, Option<FunctionValue<'ctx>>)>>,
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub llvm_types: Rc<HashMap<String, BasicTypeEnum<'ctx>>>,
    pub variables: HashMap<String, BasicValueEnum<'ctx>>,
    pub operations: Rc<HashMap<String, String>>,
}

impl<'ctx> CompilerTypeResolver<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let mut types = HashMap::new();
        let mut llvm_types = HashMap::new();

        add_primitive("f64", context.f64_type().as_basic_type_enum(), &mut types, &mut llvm_types);
        add_primitive("i64", context.i64_type().as_basic_type_enum(), &mut types, &mut llvm_types);

        return Self {
            functions: Rc::new(HashMap::new()),
            types: Rc::new(types),
            llvm_types: Rc::new(llvm_types),
            variables: HashMap::new(),
            operations: Rc::new(HashMap::new())
        }
    }

    pub fn get(&self, name: &String) -> Option<&BasicValueEnum<'ctx>> {
        return self.variables.get(name);
    }
}

impl<'ctx> TypeResolver for CompilerTypeResolver<'ctx> {
    fn get_type(&self, name: &String) -> Option<Rc<Types>> {
        return self.types.get(name).map(|tuple| tuple.clone());
    }

    fn print(&self) {
        for (static_function, _ignored) in self.functions.deref() {
            println!("{}", static_function);
        }

        for types in self.types.values() {
            println!("{}", types.structure);
        }
    }

    //TODO handle overlapping names
    fn add_operation(&mut self, operation: String, function: String) {
        unsafe { Rc::get_mut_unchecked(&mut self.operations) }.insert(operation, function);
    }

    fn get_operations(&self) -> &HashMap<String, String> {
        return &self.operations
    }

    fn get_function(&self, name: &String) -> Option<&Function> {
        return self.functions.get(name).map(|tuple| &tuple.0);
    }

    //TODO handle overlapping names
    fn add_function(&mut self, name: String, function: Function) {
        unsafe { Rc::get_mut_unchecked(&mut self.functions) }.insert(name, (function, None));
    }

    fn get_method_type(&self, name: &String, _calling: &Option<Effects>, _args: &Vec<&Effects>) -> Option<Rc<Types>> {
        return self.functions.get(name).unwrap().0.return_type.clone();
    }

    fn get_variable_type(&self, name: &String) -> Option<Rc<Types>> {
        let variable = self.variables.get(name).unwrap().get_type();
        //Reverse lookup the variable name.
        for (name, llvm_type) in self.llvm_types.deref() {
            if variable == *llvm_type {
                return Some(self.types.get(name).unwrap().clone());
            }
        }
        return None;
    }

    fn finalize(&mut self) {
        for (_name, (static_function, _ignored)) in unsafe { Rc::get_mut_unchecked(&mut self.functions.clone()) } {
            static_function.finalize(self);
        }

        let mut structures = Vec::new();
        for (_name, structure) in self.types.deref() {
            structures.push(structure.clone());
        }

        for structure in structures {
            for member in &mut unsafe { Rc::get_mut_unchecked(&mut structure.clone()) }.structure.members {
                if let TypeMembers::Function(function) = member {
                    function.finalize(self);
                }
            }
        }
    }
}


fn add_primitive<'ctx>(name: &str, primitive_type: BasicTypeEnum<'ctx>, types: &mut HashMap<String, Rc<Types>>,
                       llvm_types: &mut HashMap<String, BasicTypeEnum<'ctx>>) {
    types.insert(name.to_string(), Rc::new(Types::new_struct(
        Struct::new(Vec::new(), &[Modifier::Public], name.to_string()), None, Vec::new())));
    llvm_types.insert(name.to_string(), primitive_type);
}