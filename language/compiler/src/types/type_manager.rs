use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::types::{BasicType, BasicTypeEnum};
use ast::compiler::CompilerInfo;
use ast::Modifier;
use ast::r#struct::{Struct, TypeMembers};
use ast::type_resolver::TypeResolver;
use ast::types::Types;

pub struct TypeManager<'ctx> {
    pub types: HashMap<String, Types<'ctx>>,
    pub llvm_types: HashMap<&'ctx Types<'ctx>, BasicTypeEnum<'ctx>>
}

impl<'ctx> TypeManager<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let mut types = HashMap::new();
        let mut llvm_types = HashMap::new();
        {
            setup(&mut types, &mut llvm_types, "f64", context.f64_type().as_basic_type_enum());
        }
            setup(&mut types, &mut llvm_types,"i64", context.i64_type().as_basic_type_enum());
        return Self {
            types,
            llvm_types
        }
    }

    pub fn get_type(&'ctx self, name: &str) -> Option<&'ctx Types<'ctx>> {
        return self.types.get(name);
    }

    pub fn get_llvm_type(&self, name: &str) -> Option<&BasicTypeEnum> {
        return self.llvm_types.get(self.types.get(name)?);
    }

    pub fn get_type_err(&'ctx self, name: &str) -> &'ctx Types<'ctx> {
        return self.types.get(name).expect(format!("Unknown type {}", name).as_str());
    }
}

impl<'ctx> CompilerInfo<'ctx> for TypeManager<'ctx> {
    fn finalize_types(&mut self, type_manager: &dyn TypeResolver<'ctx>) {
        for (_name, structure) in &mut self.types {
            for member in &mut structure.structure.members {
                if let TypeMembers::Function(function) = member {
                    function.finalize(type_manager);
                }
            }
        }
    }
}

fn setup<'a>(types: &'a mut HashMap<String, Types>, llvm_types: &mut HashMap<&'a Types<'a>, BasicTypeEnum<'a>>, name: &str, basic_type: BasicTypeEnum<'a>) {
    let created_type = Types::new_struct(Struct::new(Vec::new(), &[Modifier::Public],
                                                     name.to_string()), None, Vec::new());
    types.insert(name.to_string(), created_type);
    llvm_types.insert(types.get(name).unwrap(), basic_type);
}