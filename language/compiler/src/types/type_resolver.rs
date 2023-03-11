use std::collections::HashMap;
use std::rc::Rc;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{AsTypeRef, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{BasicValueEnum, FunctionValue};
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::code::Effects;
use ast::function::Function;
use ast::{is_modifier, Modifier};
use ast::type_resolver::{FinalizedTypeResolver, TypeResolver};
use ast::types::{ResolvableTypes, Types};
use crate::internal::structs::get_internal_struct;

#[derive(Clone)]
pub struct ParserTypeResolver {
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub functions: Rc<HashMap<String, Function>>,
    pub variables: HashMap<String, ResolvableTypes>,
    pub operations: Rc<HashMap<String, Vec<String>>>,
}

impl ParserTypeResolver {
    pub fn new() -> Self {
        return Self {
            types: Rc::new(HashMap::new()),
            functions: Rc::new(HashMap::new()),
            variables: HashMap::new(),
            operations: Rc::new(HashMap::new()),
        };
    }

    pub fn finalize<'ctx>(self, context: &'ctx Context, module: &Module<'ctx>) -> CompilerTypeResolver<'ctx> {
        let mut finalized = CompilerTypeResolver::new(self.operations.clone());

        let types = Rc::try_unwrap(self.types).unwrap();
        let mut types: Vec<Rc<Types>> = types.into_values().collect();
        while !types.is_empty() {
            let found = types.pop().unwrap();
            compile(context, &found, &mut types, &mut finalized);
        }

        let finalizing = finalized.types.clone();
        let mut finalizing: Vec<&Rc<Types>> = finalizing.values().clone().collect();
        while !finalizing.is_empty() {
            compile_llvm_type(context, finalizing.pop().unwrap(), &mut finalizing, &mut finalized);
        }

        finalized.setup_functions(context, module, Rc::try_unwrap(self.functions).unwrap());

        return finalized;
    }
}

pub fn compile_llvm_type<'ctx>(context: &'ctx Context, types: &Rc<Types>, all: &mut Vec<&Rc<Types>>, finalized: &mut CompilerTypeResolver<'ctx>) {
    finalized.start_struct(types.clone());
    unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.structure.finalize(finalized);
    finalized.end_struct();

    if is_modifier(types.structure.modifiers, Modifier::Internal) {
        let (size, llvm_type) = get_internal_struct(context, &types.name);
        unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }
            .insert(types.clone(), llvm_type);
        unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.size = size;
    } else {
        let opaque_type = context.opaque_struct_type(&types.structure.name);
        unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }.insert(types.clone(), opaque_type.as_basic_type_enum());
        let mut llvm_fields = Vec::new();
        for field in types.get_fields() {
            let field_type = field.field.field_type.unwrap();
            match finalized.llvm_types.get(field_type) {
                Some(found_type) => llvm_fields.push(*found_type),
                None => {
                    let position = all.iter().position(|found| *found == field_type).unwrap();
                    compile_llvm_type(context, all.remove(position), all, finalized);
                    llvm_fields.push(*finalized.llvm_types.get(field.field.field_type.unwrap()).unwrap())
                }
            }
        }
        opaque_type.set_body(llvm_fields.as_slice(), false);

        let mut size = 0;
        for field in types.get_fields() {
            size += field.field.field_type.unwrap().size;
        }
        unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.size = size;
    };
}

pub fn compile<'ctx>(context: &'ctx Context, types: &Rc<Types>, all: &mut Vec<Rc<Types>>, finalizer: &mut CompilerTypeResolver<'ctx>) {
    for found in &types.traits {
        match found {
            ResolvableTypes::Resolving(name) => {
                let position = all.iter().position(|temp| &temp.name == name).unwrap();
                compile(context, &all.remove(position), all, finalizer);
            }
            _ => {}
        }
    }

    if types.parent.is_some() {
        let parent = types.parent.as_ref().unwrap();

        match parent {
            ResolvableTypes::Resolving(name) => {
                let position = all.iter().position(|temp| &temp.name == name).unwrap();
                compile(context, &all.remove(position), all, finalizer);
            }
            _ => {}
        }
    }

    if types.structure.fields.is_some() {
        for field in types.structure.fields.as_ref().unwrap() {
            match &field.field.field_type {
                ResolvableTypes::Resolving(name) => {
                    if name == "Self" {
                        panic!("Can't have Self in a field!");
                    }
                    match all.iter().position(|temp| &temp.name == name) {
                        Some(pos) => compile(context, &all.remove(pos), all, finalizer),
                        None => {}
                    }
                }
                _ => {}
            }
        }
    }

    unsafe { Rc::get_mut_unchecked(&mut finalizer.types) }.insert(types.name.clone(), types.clone());
}

impl TypeResolver for ParserTypeResolver {
    fn add_type(&mut self, adding_types: Rc<Types>) {
        unsafe { Rc::get_mut_unchecked(&mut self.types) }.insert(adding_types.name.clone(), adding_types);
    }

    fn add_function(&mut self, function: Function) {
        unsafe { Rc::get_mut_unchecked(&mut self.functions) }.insert(function.name.clone(), function);
    }

    fn get_function(&self, name: &String) -> &Function {
        return self.functions.get(name).unwrap();
    }

    fn add_operation(&mut self, name: String, function: String) {
        match unsafe { Rc::get_mut_unchecked(&mut self.operations) }.get_mut(&name) {
            Some(functions) => functions.push(function),
            None => {
                unsafe { Rc::get_mut_unchecked(&mut self.operations) }.insert(name, vec!(function));
            }
        }
    }
}

#[derive(Clone)]
pub struct CompilerTypeResolver<'ctx> {
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub functions: Rc<HashMap<String, (Function, FunctionValue<'ctx>)>>,
    pub llvm_types: Rc<HashMap<Rc<Types>, BasicTypeEnum<'ctx>>>,
    pub variables: HashMap<String, BasicValueEnum<'ctx>>,
    pub operations: Rc<HashMap<String, Vec<String>>>,
    pub current_struct: Option<Rc<Types>>,
}

impl<'ctx> CompilerTypeResolver<'ctx> {
    pub fn new(operations: Rc<HashMap<String, Vec<String>>>) -> Self {
        let llvm_types = HashMap::new();

        return Self {
            types: Rc::new(HashMap::new()),
            functions: Rc::new(HashMap::new()),
            llvm_types: Rc::new(llvm_types),
            variables: HashMap::new(),
            operations,
            current_struct: None,
        };
    }

    fn start_struct(&mut self, structure: Rc<Types>) {
        self.current_struct = Some(structure);
    }

    fn end_struct(&mut self) {
        self.current_struct = None;
    }

    fn setup_functions(&mut self, context: &'ctx Context, module: &Module<'ctx>, functions: HashMap<String, Function>) {
        let mut new_functions = HashMap::new();
        for (name, mut function) in functions {
            function.finalize(self);
            let func_value = get_func_value(&function, module, context, &self.llvm_types);
            new_functions.insert(name, (function, func_value));
        }

        self.functions = Rc::new(new_functions);

        for (_name, (function, _func_value)) in unsafe { Rc::get_mut_unchecked(&mut self.functions.clone()) } {
            function.finalize_code(self);
        }
    }

    pub fn print(&self) {
        for types in self.types.values() {
            print!("{}\n\n", types.structure);
        }

        for function in self.functions.values() {
            print!("{}\n\n", function.0);
        }
    }
}

impl<'ctx> FinalizedTypeResolver for CompilerTypeResolver<'ctx> {
    fn finalize(&self, resolving: &mut ResolvableTypes) {
        match resolving {
            ResolvableTypes::Resolving(name) =>
                {
                    if name == "Self" {
                        *resolving = ResolvableTypes::Resolved(
                            self.current_struct.as_ref().expect("Can't use Self outside of a struct!").clone())
                    } else {
                        *resolving = ResolvableTypes::Resolved(self.types.get(name).expect(
                            format!("Unknown type {}", name).as_str()).clone())
                    }
                }
            ResolvableTypes::Resolved(_) => panic!("Tried to resolve already-resolved type!")
        }
    }

    fn get_operator(&self, effects: &Vec<Effects>, operator: String) -> Option<&Function> {
        for operation in self.operations.get(&operator).unwrap() {
            let function: &Function = &self.functions.get(operation).as_ref().unwrap().0;
            if function.fields.len() != effects.len() {
                continue;
            }

            for i in 0..effects.len() {
                if function.fields.get(i).unwrap().field_type !=
                    effects.get(i).unwrap().unwrap().return_type().unwrap() {
                    continue;
                }
            }

            return Some(function);
        }
        return None;
    }

    fn get_function(&self, name: &String) -> Option<&Function> {
        return self.functions.get(name).map(|func| &func.0);
    }
}

fn get_func_value<'ctx>(function: &Function, module: &Module<'ctx>, context: &'ctx Context,
                        llvm_types: &HashMap<Rc<Types>, BasicTypeEnum<'ctx>>) -> FunctionValue<'ctx> {
    let return_type = match &function.return_type {
        Some(found) => llvm_types.get(found.unwrap()).unwrap().as_type_ref(),
        None => context.void_type().as_type_ref()
    };

    let mut params: Vec<LLVMTypeRef> = function.fields.iter().map(
        |field| llvm_types.get(field.field_type.unwrap()).unwrap().as_type_ref()).collect();

    let fn_type = unsafe {
        FunctionType::new(LLVMFunctionType(return_type, params.as_mut_ptr(),
                                           params.len() as u32, false as i32))
    };

    return module.add_function(function.name.as_str(), fn_type, None);
}