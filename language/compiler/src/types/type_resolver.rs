use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{AsTypeRef, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, GlobalValue};
use llvm_sys::core::LLVMFunctionType;
use ast::code::{Effect, Effects};
use ast::function::Function;
use ast::{DisplayIndented, is_modifier, Modifier};
use ast::type_resolver::{FinalizedTypeResolver, TypeResolver};
use ast::types::{ResolvableTypes, Types};
use crate::internal::structs::get_internal_struct;

#[derive(Clone)]
pub struct ParserTypeResolver {
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub functions: Rc<HashMap<String, Function>>,
    pub operations: Rc<HashMap<String, Vec<String>>>,
}

impl ParserTypeResolver {
    pub fn new() -> Self {
        return Self {
            types: Rc::new(HashMap::new()),
            functions: Rc::new(HashMap::new()),
            operations: Rc::new(HashMap::new()),
        };
    }

    pub fn finalize<'ctx>(self, context: &'ctx Context, module: &Module<'ctx>) -> CompilerTypeResolver<'ctx> {
        let mut finalized = CompilerTypeResolver::new(context, self.operations.clone());

        //Compile types
        let types = Rc::try_unwrap(self.types).unwrap();
        let mut types: Vec<Rc<Types>> = types.into_values().collect();
        while !types.is_empty() {
            let found = types.pop().unwrap();
            compile(context, &found, &mut types, &mut finalized);
        }

        //Finalize LLVM types
        let finalizing = finalized.types.clone();
        let mut finalizing: Vec<&Rc<Types>> = finalizing.values().clone().collect();
        while !finalizing.is_empty() {
            compile_llvm_type(context, module, finalizing.pop().unwrap(), &mut finalizing, &mut finalized);
        }

        //Setup vtables
        for types in finalized.types.values() {
            let (_llvm_type, vtable) = unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }.get_mut(types).unwrap();
            let functions = types.structure.as_ref().unwrap().functions.iter()
                .map(|function| finalized.functions.get(function).unwrap().1).collect();
            let raw_table = create_vtable(context, functions);
            unsafe { vtable.delete() };
            *vtable = module.add_global(raw_table.get_type(), None, &types.name);
            vtable.set_initializer(&raw_table);
        }

        //Setup functions
        finalized.setup_functions(context, module, Rc::try_unwrap(self.functions).unwrap());

        return finalized;
    }
}

pub fn compile_llvm_type<'ctx>(context: &'ctx Context, module: &Module<'ctx>, types: &Rc<Types>,
                               all: &mut Vec<&Rc<Types>>, finalized: &mut CompilerTypeResolver<'ctx>) {
    unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.structure.as_mut().unwrap().finalize(finalized);

    if is_modifier(types.structure.as_ref().unwrap().modifiers, Modifier::Internal) {
        let (size, llvm_type) = get_internal_struct(context, &types.name);
        let vtable = module.add_global(context.i64_type(), None, &types.name);

        unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }
            .insert(types.clone(), (llvm_type, vtable));
        unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.size = size;
    } else {
        let opaque_type = context.opaque_struct_type(&types.structure.as_ref().unwrap().name);

        //Give it a temp vtable
        unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }
            .insert(types.clone(), (opaque_type.as_basic_type_enum(), module.add_global(context.i64_type(), None, &types.name)));

        let mut llvm_fields = vec!(context.i64_type().ptr_type(AddressSpace::default()).as_basic_type_enum());

        for field in types.get_fields().unwrap() {
            let field_type = field.field.field_type.unwrap();
            match finalized.llvm_types.get(field_type) {
                Some(found_type) => llvm_fields.push(found_type.0),
                None => {
                    let position = all.iter().position(|found| *found == field_type).unwrap();
                    compile_llvm_type(context, module, all.remove(position), all, finalized);
                    llvm_fields.push(finalized.llvm_types.get(field.field.field_type.unwrap()).unwrap().0)
                }
            }
        }
        opaque_type.set_body(llvm_fields.as_slice(), false);

        let mut size = 0;
        for field in types.get_fields().unwrap() {
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

    if types.structure.as_ref().unwrap().fields.is_some() {
        for field in types.structure.as_ref().unwrap().fields.as_ref().unwrap() {
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
    pub context: &'ctx Context,
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub functions: Rc<HashMap<String, (Function, FunctionValue<'ctx>)>>,
    pub llvm_types: Rc<HashMap<Rc<Types>, (BasicTypeEnum<'ctx>, GlobalValue<'ctx>)>>,
    pub operations: Rc<HashMap<String, Vec<String>>>,
    pub func_types: HashMap<String, (Rc<Types>, BasicTypeEnum<'ctx>)>,
    pub variables: HashMap<String, (Rc<Types>, BasicValueEnum<'ctx>)>
}

impl<'ctx> CompilerTypeResolver<'ctx> {
    pub fn new(context: &'ctx Context, operations: Rc<HashMap<String, Vec<String>>>) -> Self {
        let llvm_types = HashMap::new();

        return Self {
            context,
            types: Rc::new(HashMap::new()),
            functions: Rc::new(HashMap::new()),
            llvm_types: Rc::new(llvm_types),
            operations,
            func_types: HashMap::new(),
            variables: HashMap::new()
        };
    }

    pub(crate) fn for_func(&self, function: &String) -> Self {
        let mut type_manager = self.clone();
        let (function, function_value) = self.functions.get(function).unwrap();

        for generic in &function.generics {
            type_manager.func_types.insert(generic.name().clone(),
                                           (generic.unwrap().clone(), self.context.struct_type(&[], false).as_basic_type_enum()));
        }
        let params = function_value.get_params();
        let offset = params.len() != function.fields.len();
        for i in 0..function.fields.len() {
            let field = function.fields.get(i).unwrap();
            if offset {
                type_manager.variables.insert(field.name.clone(),
                                              (field.field_type.unwrap().clone(), params.get(i+1).unwrap().clone()));
            } else {
                type_manager.variables.insert(field.name.clone(),
                                              (field.field_type.unwrap().clone(), params.get(i).unwrap().clone()));
            }
        }
        return type_manager;
    }

    fn setup_functions(&mut self, context: &'ctx Context, module: &Module<'ctx>, mut functions: HashMap<String, Function>) {
        let mut new_functions = HashMap::new();
        for (_, structure) in unsafe { Rc::get_mut_unchecked(&mut self.types.clone()) } {
            if structure.structure.is_none() {
                continue;
            }

            for name in &mut unsafe { Rc::get_mut_unchecked(structure) }.structure.as_mut().unwrap().functions {
                let mut function = functions.remove(name).unwrap();
                function.finalize(self);
                let func_value = get_func_value(&function, module, context, &self.llvm_types);
                new_functions.insert(function.name.clone(), (function, func_value));
            }
        }

        for (name, mut function) in functions {
            function.finalize(self);
            let func_value = get_func_value(&function, module, context, &self.llvm_types);
            new_functions.insert(name, (function, func_value));
        }

        self.functions = Rc::new(new_functions);

        for (_name, (function, _func_value)) in unsafe { Rc::get_mut_unchecked(&mut self.functions.clone()) } {
            let mut type_manager = self.clone();
            function.finalize_code(&mut type_manager);
        }
    }
}

impl<'ctx> Display for CompilerTypeResolver<'ctx> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for types in self.types.values() {
            if types.structure.is_some() {
                types.structure.as_ref().unwrap().format("", f, self)?;
                write!(f, "\n\n")?;
            }
        }

        for function in self.functions.values() {
            function.0.format("", f)?;
            write!(f, "\n\n")?;
        }

        return Ok(());
    }
}

impl<'ctx> FinalizedTypeResolver for CompilerTypeResolver<'ctx> {
    fn finalize(&self, resolving: &mut ResolvableTypes) {
        match resolving {
            ResolvableTypes::Resolving(name) =>
                {
                    match self.types.get(name) {
                        Some(found) => *resolving = ResolvableTypes::Resolved(found.clone()),
                        None => match self.func_types.get(name) {
                            Some((temp_type, _)) => *resolving = ResolvableTypes::Resolved(temp_type.clone()),
                            None => panic!("Unknown type {}!", name)
                        }
                    }
                }
            ResolvableTypes::ResolvingGeneric(name, bounds) => {
                let mut type_bounds = Vec::new();
                let mut parent = None;
                for bound in bounds {
                    let found = self.types.get(bound).expect(format!("Unknown type {}", bound).as_str());
                    if found.is_trait {
                        type_bounds.push(ResolvableTypes::Resolved(found.clone()));
                    } else {
                        if parent.is_some() {
                            let unwrapped: &ResolvableTypes = parent.as_ref().unwrap();
                            if unwrapped.unwrap().has_parent(found) {
                                //Already good
                            } else if found.has_parent(unwrapped.unwrap()) {
                                parent = Some(parent.unwrap());
                            } else {
                                panic!("Two parents for single generic! {} and {}", unwrapped, found)
                            }
                        } else {
                            parent = Some(ResolvableTypes::Resolved(found.clone()));
                        }
                    }
                }
                *resolving = ResolvableTypes::Resolved(Rc::new(Types::new_generic(name.clone(), parent, type_bounds)))
            }
            ResolvableTypes::Resolved(_) => panic!("Tried to resolve already-resolved type!")
        }
    }

    fn finalize_func(&mut self, function: &mut Function) {
        let mut type_manager = self.clone();

        for generic in &function.generics {
            type_manager.func_types.insert(generic.name().clone(), (generic.unwrap().clone(),
                                                            self.context.struct_type(&[], false).as_basic_type_enum()));
        }

        if function.return_type.is_some() {
            function.return_type.as_mut().unwrap().finalize(&mut type_manager);
        }

        for field in &mut function.fields {
            field.finalize(&mut type_manager);
        }
    }

    fn finalize_code(&mut self, function: &String) {
        let mut temp = self.functions.clone();
        let (func, _func_value) = unsafe { Rc::get_mut_unchecked(&mut temp) }.get_mut(function).unwrap();
        func.code.finalize(&mut self.for_func(function));
    }

    fn get_variable(&self, name: &String) -> Option<ResolvableTypes> {
        return self.variables.get(name).map(|found| ResolvableTypes::Resolved(found.0.clone()));
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
                        llvm_types: &HashMap<Rc<Types>, (BasicTypeEnum<'ctx>, GlobalValue<'ctx>)>) -> FunctionValue<'ctx> {
    let mut params = Vec::new();

    let mut return_type = context.void_type().as_type_ref();

    if function.return_type.is_some() {
        let found_type = match llvm_types.get(&function.return_type.as_ref().unwrap().unwrap().clone()) {
            Some(llvm_type) => llvm_type.0,
            //Generics get an opaque type
            None => context.struct_type(&[], false).as_basic_type_enum()
        };
        if found_type.is_struct_type() {
            params.push(found_type.ptr_type(AddressSpace::default()).as_type_ref());
        } else {
            return_type = found_type.as_type_ref();
        }
    }

    for field in &function.fields {
        params.push(match llvm_types.get(field.field_type.unwrap()) {
            Some(llvm_type) => llvm_type.0,
            None => context.struct_type(&[], false).as_basic_type_enum()
        }.as_type_ref());
    }

    let fn_type = unsafe {
        FunctionType::new(LLVMFunctionType(return_type, params.as_mut_ptr(),
                                           params.len() as u32, false as i32))
    };

    return module.add_function(function.name.as_str(), fn_type, None);
}

fn create_vtable<'ctx>(context: &'ctx Context, functions: Vec<FunctionValue<'ctx>>) -> BasicValueEnum<'ctx> {
    let mut tables = Vec::new();
    for function in &functions {
        tables.push(function.as_global_value().as_basic_value_enum());
    }
    return context.const_struct(tables.as_slice(), false).as_basic_value_enum();
}