use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{AsTypeRef, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, GlobalValue};
use llvm_sys::core::LLVMFunctionType;
use ast::code::{Effect, Effects};
use ast::function::{display, Function};
use ast::{DisplayIndented, is_modifier, Modifier};
use ast::r#struct::Struct;
use ast::type_resolver::{FinalizedTypeResolver, TypeResolver};
use ast::types::{ResolvableTypes, Types};
use crate::internal::structs::get_internal_struct;
use crate::types::resolved_type::ResolvedType;

#[derive(Clone)]
pub struct ParserTypeResolver {
    pub imports: Rc<HashMap<String, HashMap<String, String>>>,
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub types_by_file: Rc<HashMap<String, Vec<Rc<Types>>>>,
    pub functions: Rc<HashMap<String, Function>>,
    pub operations: Rc<HashMap<String, Vec<String>>>,
    pub unresolved: Rc<HashMap<String, (String, String, Vec<Function>)>>,
}

impl ParserTypeResolver {
    pub fn new() -> Self {
        return Self {
            imports: Rc::new(HashMap::new()),
            types: Rc::new(HashMap::new()),
            types_by_file: Rc::new(HashMap::new()),
            functions: Rc::new(HashMap::new()),
            operations: Rc::new(HashMap::new()),
            unresolved: Rc::new(HashMap::new()),
        };
    }

    pub fn finalize<'a, 'ctx>(mut self, context: &'ctx Context, module: &'a Module<'ctx>) -> CompilerTypeResolver<'a, 'ctx> {
        let mut finalized = CompilerTypeResolver::new(context, module,
                                                      self.operations.clone(), self.imports.clone());
        //Compile types
        let types = Rc::try_unwrap(self.types).unwrap();
        let mut types: Vec<Rc<Types>> = types.into_values().collect();
        while !types.is_empty() {
            let found = types.pop().unwrap();
            compile(context, &found, &mut types, &mut finalized);
        }

        //Resolve types
        for (file, (base, mut implementing, functions)) in Rc::try_unwrap(self.unresolved).unwrap() {
            let mut found_base = None;
            let mut found_implementing = None;
            match self.imports.get(&file) {
                Some(imports) => for (name, full_name) in imports {
                    if &base == name {
                        found_base = Some(ResolvableTypes::Resolving(full_name.clone()));
                    }
                    if &implementing == name {
                        found_implementing = Some(ResolvableTypes::Resolving(full_name.clone()));
                    }
                },
                None => {}
            }

            for possible in self.types_by_file.get(&file).unwrap() {
                let testing = possible.name.split("::").last().unwrap();
                if testing == base {
                    found_base = Some(ResolvableTypes::Resolved(possible.clone()));
                }
                if testing == implementing {
                    found_implementing = Some(ResolvableTypes::Resolved(possible.clone()));
                }
            }

            let mut out_base;
            if let Some(base) = found_base {
                out_base = base;
            } else {
                out_base = ResolvableTypes::Resolving(base.clone());
            }
            out_base.finalize(&mut finalized);

            let mut out_implementing;
            if let Some(implementing) = found_implementing {
                out_implementing = implementing;
            } else {
                out_implementing = ResolvableTypes::Resolving(implementing.clone());
            }
            out_implementing.finalize(&mut finalized);

            for testing in &out_implementing.unwrap().structure.functions {
                let name = testing.split("::").last().unwrap();
                if !functions.iter().any(|found| found.name.split("::").last().unwrap() == name) {
                    panic!("Missing implementation for function {} for struct {}", name, base);
                }
            }

            if functions.len() != out_implementing.unwrap().structure.functions.len() {
                panic!("Too many functions implemented!")
            }

            let mut found = out_base.unwrap().clone();
            let mut found = unsafe { Rc::get_mut_unchecked(&mut found) };
            for function in functions {
                found.structure.functions.push(function.name.clone());
                unsafe { Rc::get_mut_unchecked(&mut self.functions) }.insert(function.name.clone(), function);
            }

            found.traits.push(out_implementing);
        }

        //Finalize LLVM types
        let finalizing = finalized.types.clone();
        let mut finalizing: Vec<&Rc<Types>> = finalizing.values().clone().collect();
        while !finalizing.is_empty() {
            compile_llvm_type(context, module, finalizing.pop().unwrap(), &mut finalizing, &mut finalized);
        }

        //Setup vtables
        for types in finalized.types.values() {
            if !types.structure.generics.is_empty() {
                continue;
            }
            for (traits, vtable) in &mut unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }.get_mut(types).unwrap().vtables {
                let functions = types.structure.functions.iter()
                    .filter(|function| traits.structure.functions.contains(function))
                    .map(|function| finalized.functions.get(function).unwrap().1).collect();
                let raw_table = create_vtable(context, functions);
                unsafe { vtable.delete() };
                *vtable = module.add_global(raw_table.get_type(), None, &types.name);
                vtable.set_initializer(&raw_table);
            }
        }

        //Setup functions
        finalized.setup_functions(context, module, Rc::try_unwrap(self.functions).unwrap());

        return finalized;
    }
}

pub fn compile_llvm_type<'a, 'ctx>(context: &'ctx Context, module: &Module<'ctx>, types: &Rc<Types>,
                                   all: &mut Vec<&Rc<Types>>, finalized: &mut CompilerTypeResolver<'a, 'ctx>) {
    if !types.structure.generics.is_empty() {
        unsafe { Rc::get_mut_unchecked(&mut finalized.generic_types) }.insert(types.name.clone(), types.clone());
        return;
    }

    unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.structure.finalize(finalized);

    if is_modifier(types.structure.modifiers, Modifier::Internal) {
        let (size, llvm_type) = get_internal_struct(context, &types.name);

        unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }
            .insert(types.clone(), ResolvedType::new(llvm_type, HashMap::new()));
        unsafe { Rc::get_mut_unchecked(&mut types.clone()) }.size = size;
    } else {
        let opaque_type = context.opaque_struct_type(&types.structure.name);

        let vtables = HashMap::new();
        for found_trait in &types.traits {
            module.add_global(context.i64_type(), None, &(types.name.clone() + "_" + found_trait.name()));
        }

        //Give it a temp vtable
        unsafe { Rc::get_mut_unchecked(&mut finalized.llvm_types) }
            .insert(types.clone(), ResolvedType::new(opaque_type.as_basic_type_enum(), vtables));
        let mut llvm_fields = vec!(context.i64_type().ptr_type(AddressSpace::default()).as_basic_type_enum());

        for field in types.get_fields() {
            let field_type = field.field.field_type.unwrap();
            match finalized.llvm_types.get(field_type) {
                Some(found_type) => llvm_fields.push(found_type.types),
                None => {
                    let position = all.iter().position(|found| *found == field_type).unwrap();
                    compile_llvm_type(context, module, all.remove(position), all, finalized);
                    llvm_fields.push(finalized.llvm_types.get(field.field.field_type.unwrap()).unwrap().types)
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

pub fn compile<'a, 'ctx>(context: &'ctx Context, types: &Rc<Types>, all: &mut Vec<Rc<Types>>,
                         finalizer: &mut CompilerTypeResolver<'a, 'ctx>) {
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
    fn add_import(&mut self, file: &String, importing: String) {
        let mut temp = unsafe { Rc::get_mut_unchecked(&mut self.imports) };
        match temp.get_mut(file) {
            Some(found) => {
                found.insert(importing.split(":").last().unwrap().to_string(), importing);
            }
            None => {
                let mut imports = HashMap::new();
                imports.insert(importing.split(":").last().unwrap().to_string(), importing);
                temp.insert(file.clone(), imports);
            }
        }
    }

    fn add_type(&mut self, adding_types: Rc<Types>) {
        unsafe { Rc::get_mut_unchecked(&mut self.types) }.insert(adding_types.name.clone(), adding_types.clone());
        if !adding_types.name.contains("::") {
            return;
        }
        let file = adding_types.name[..adding_types.name.len() - 2 - adding_types.name.split("::").last().unwrap().len()].to_string();
        match unsafe { Rc::get_mut_unchecked(&mut self.types_by_file) }.get_mut(&file) {
            Some(vec) => vec.push(adding_types),
            None => {
                unsafe { Rc::get_mut_unchecked(&mut self.types_by_file) }.insert(file, vec!(adding_types));
            }
        }
    }

    fn add_unresolved_type(&mut self, file: String, base: String, implementing: String, functions: Vec<Function>) {
        unsafe { Rc::get_mut_unchecked(&mut self.unresolved) }.insert(file, (base, implementing, functions));
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
pub struct CompilerTypeResolver<'a, 'ctx> {
    pub context: &'ctx Context,
    pub module: &'a Module<'ctx>,
    pub types: Rc<HashMap<String, Rc<Types>>>,
    pub functions: Rc<HashMap<String, (Function, FunctionValue<'ctx>)>>,
    pub llvm_types: Rc<HashMap<Rc<Types>, ResolvedType<'ctx>>>,
    pub operations: Rc<HashMap<String, Vec<String>>>,
    pub generics: Rc<HashMap<String, Function>>,
    pub generic_types: Rc<HashMap<String, Rc<Types>>>,
    pub func_types: HashMap<String, ResolvableTypes>,
    pub variables: HashMap<String, (Rc<Types>, Option<BasicValueEnum<'ctx>>)>,
    pub imports: Rc<HashMap<String, HashMap<String, String>>>,
    pub current_file: Vec<String>,
}

impl<'a, 'ctx> CompilerTypeResolver<'a, 'ctx> {
    pub fn new(context: &'ctx Context, module: &'a Module<'ctx>, operations: Rc<HashMap<String, Vec<String>>>,
               imports: Rc<HashMap<String, HashMap<String, String>>>) -> Self {
        let llvm_types = HashMap::new();

        return Self {
            context,
            module,
            types: Rc::new(HashMap::new()),
            functions: Rc::new(HashMap::new()),
            llvm_types: Rc::new(llvm_types),
            operations,
            generics: Rc::new(HashMap::new()),
            generic_types: Rc::new(HashMap::new()),
            func_types: HashMap::new(),
            variables: HashMap::new(),
            imports,
            current_file: Vec::new(),
        };
    }

    fn add_type(&mut self, original: &Rc<Types>, types: Struct) -> Rc<Types> {
        let output = Rc::new(Types::new_struct(types, original.parent.clone(), original.traits.clone()));
        unsafe { Rc::get_mut_unchecked(&mut self.types) }.insert(output.name.clone(), output.clone());
        //All is empty because all dependencies of this structure are already compiled
        compile_llvm_type(self.context, self.module, &output, &mut Vec::new(), self);
        return output;
    }

    pub fn get_llvm_type(&self, types: &Rc<Types>) -> &BasicTypeEnum<'ctx> {
        return match self.llvm_types.get(types) {
            Some(llvm_type) => &llvm_type.types,
            None => panic!("Failed to get LLVM type!")
        };
    }

    pub fn for_func(&self, function: &String) -> Self {
        let mut type_manager = self.clone();
        let (function, function_value) = self.functions.get(function).unwrap();

        let params = function_value.get_params();
        let mut offset = (params.len() != function.fields.len()) as usize;
        for i in 0..function.fields.len() {
            let field = function.fields.get(i).unwrap();
            type_manager.variables.insert(field.name.clone(), (field.field_type.unwrap().clone(),
                                                               Some((*params.get(i + offset).unwrap()).clone())));
        }
        return type_manager;
    }

    fn setup_functions(&mut self, context: &'ctx Context, module: &Module<'ctx>, mut functions: HashMap<String, Function>) {
        let mut new_functions = HashMap::new();
        for (name, mut function) in functions {
            if !function.generics.is_empty() {
                for found in function.generics.values_mut() {
                    for types in found {
                        types.finalize(self);
                    }
                }
                unsafe { Rc::get_mut_unchecked(&mut self.generics) }.insert(name, function);
                continue;
            }
            let all: Vec<&str> = function.name.split("::").collect();
            let all = &all[0..all.len() - 2].to_vec();
            function.finalize(Some(&display(all, "::")), self);
            let func_value = get_func_value(&function, module, context, &self.llvm_types);
            new_functions.insert(name, (function, func_value));
        }

        self.functions = Rc::new(new_functions);

        let mut temp = self.functions.clone();
        let mut all_funcs = HashMap::new();

        for (key, (func, _)) in unsafe { Rc::get_mut_unchecked(&mut temp) } {
            all_funcs.insert(key.clone(), func);
        }

        for structure in self.types.values() {
            for function in &structure.structure.functions {
                let found = match all_funcs.remove(function) {
                    Some(func) => func,
                    None => continue
                };
                let mut type_manager = self.clone();
                let all: Vec<&str> = function.split("::").collect();
                let all = &all[0..all.len() - 2].to_vec();
                found.finalize_code(Some(&display(all, "::")), &mut type_manager);
            }
        }

        for function in all_funcs.values_mut() {
            if !function.generics.is_empty() {
                continue;
            }
            let mut type_manager = self.clone();
            function.finalize_code(None, &mut type_manager);
        }
    }

    fn check_func_import<'b, T>(&self, name: &String, input: &'b HashMap<String, T>) -> Option<&'b T> {
        let end_name = name.split("::").last().unwrap();
        if end_name.len() == name.len() {
            return input.get(name);
        }
        let parent = &name[0..name.len() - end_name.len() - 2].to_string();
        return if parent.contains(":") {
            input.get(name)
        } else {
            match self.get_import(parent) {
                Some(import) => {
                    let name = import.clone() + "::" + end_name;
                    input.get(&name)
                }
                None => {
                    for file in &self.current_file {
                        if let Some(found) = input.get(&(file.clone() + "::" + end_name)) {
                            return Some(found);
                        }
                    }
                    return input.get(name);
                }
            }
        };
    }

    fn check_import<'b, T>(&self, name: &String, input: &'b HashMap<String, T>) -> Option<&'b T> where T: Debug {
        return if name.contains(":") {
            input.get(name)
        } else {
            match self.get_import(name) {
                Some(import) => {
                    let name = import.clone();
                    input.get(&name)
                }
                None => {
                    for file in &self.current_file {
                        if let Some(found) = input.get(&(file.clone() + "::" + name)) {
                            return Some(found);
                        }
                    }
                    return input.get(name);
                }
            }
        };
    }
}

impl<'a, 'ctx> Display for CompilerTypeResolver<'a, 'ctx> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for types in self.types.values() {
            types.structure.format("", f, self)?;
            write!(f, "\n\n")?;
        }

        for function in self.functions.values() {
            function.0.format("", f)?;
            write!(f, "\n\n")?;
        }

        return Ok(());
    }
}

impl<'a, 'ctx> FinalizedTypeResolver for CompilerTypeResolver<'a, 'ctx> {
    fn start_file(&mut self, names: Vec<String>) {
        if self.current_file.is_empty() {
            self.current_file = names;
        }
    }

    fn get_struct(&self, name: &String) -> Option<&Struct> {
        return self.types.get(name).map(|found| &found.structure);
    }

    fn get_import(&self, name: &String) -> Option<&String> {
        for file in &self.current_file {
            if let Some(found) = self.imports.get(file) {
                return found.get(name);
            };
        }
        return None;
    }

    fn solidify_generics(&mut self, function: &String, generics: &HashMap<String, ResolvableTypes>) -> &Function {
        let mut output;
        {
            let temp = self.generics.clone();
            self.func_types = generics.clone();
            let func = self.check_func_import(function, temp.deref()).unwrap();
            output = func.set_generics(self, &generics);
        }

        output.generics.clear();
        let all: Vec<&str> = function.split("::").collect();
        let all = &all[0..all.len() - 2].to_vec();
        output.finalize(Some(&display(all, "::")), self);

        let name = output.name.clone();
        let func_val = get_func_value(&output, &self.module, self.context, &self.llvm_types);
        unsafe { Rc::get_mut_unchecked(&mut self.functions) }.insert(name.clone(), (output, func_val));

        unsafe { Rc::get_mut_unchecked(&mut self.functions.clone()) }.get_mut(&name).unwrap().0.finalize_code(None, self);
        self.func_types.clear();

        return &self.functions.get(&name).unwrap().0;
    }

    fn finalize(&mut self, resolving: &mut ResolvableTypes) {
        match resolving {
            ResolvableTypes::Resolving(name) =>
                {
                    match self.types.get(name) {
                        Some(found) => *resolving = ResolvableTypes::Resolved(found.clone()),
                        None => match self.check_import(name, &self.func_types) {
                            Some(temp_type) => *resolving = temp_type.clone(),
                            None => match self.check_import(&name.split("<").next().unwrap().to_string(), &self.generic_types) {
                                Some(generic) => {
                                    let generic = generic.clone();
                                    let generics: Vec<String> = name[name.split("<").next().unwrap().len() + 1..name.len() - 1].split(",")
                                        .map(|string| string.to_string()).collect();
                                    let name = generic.structure.get_mangled_name(&generics);
                                    if let Some(found) = self.types.get(&name) {
                                        *resolving = ResolvableTypes::Resolved(found.clone());
                                        return;
                                    }
                                    let mut generics = generics.iter().map(
                                        |generic| ResolvableTypes::Resolving(generic.to_string())).collect();
                                    for generic in &mut generics {
                                        self.finalize(generic);
                                    }

                                    let mut types = generic.structure.resolve_generics(self, &generics);
                                    types.generics.clear();
                                    let types = self.add_type(&generic, types);
                                    let mut iter = ResolvableTypes::Resolved(self.types.get("iter::Iter").unwrap().clone());
                                    if types.traits.contains(&iter) {
                                        let mut temp_generics = HashMap::new();
                                        temp_generics.insert("T".to_string(), generics.get(0).unwrap().clone());
                                        self.solidify_generics(types.structure.functions
                                                                   .iter().find(|func| func.contains("is_end")).unwrap(), &temp_generics);
                                        self.solidify_generics(types.structure.functions
                                                                   .iter().find(|func| func.contains("next")).unwrap(), &temp_generics);
                                    }
                                    *resolving = ResolvableTypes::Resolved(types);
                                }
                                None => {
                                    panic!("Unknown type {}!", name)
                                }
                            }
                        }
                    }
                }
            ResolvableTypes::Resolved(_) => {}
        }
    }

    fn finalize_func(&mut self, function: &mut Function) {
        let mut type_manager = self.clone();

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

    fn get_generic_struct(&self, name: &String) -> Option<&Rc<Types>> {
        return self.generic_types.get(name);
    }

    fn get_variable(&self, name: &String) -> Option<ResolvableTypes> {
        return self.variables.get(name).map(|found| ResolvableTypes::Resolved(found.0.clone()));
    }

    fn add_variable(&mut self, name: String, types: ResolvableTypes) {
        self.variables.insert(name, (types.unwrap().clone(), None));
    }

    fn get_operator(&self, effects: &Vec<Effects>, operator: String) -> Option<&Function> {
        for operation in self.operations.get(&operator)
            .expect(format!("Couldn't find operator {}", operator).as_str()) {
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
        return match self.check_func_import(name, self.functions.deref()) {
            Some(found) => Some(&found.0),
            None => self.check_func_import(name, self.generics.deref())
        };
    }
}

pub fn get_func_value<'ctx>(function: &Function, module: &Module<'ctx>, context: &'ctx Context,
                            llvm_types: &HashMap<Rc<Types>, ResolvedType<'ctx>>) -> FunctionValue<'ctx> {
    let mut params = Vec::new();

    let mut return_type = context.void_type().as_type_ref();

    if function.return_type.is_some() {
        let found_type = match llvm_types.get(&function.return_type.as_ref().unwrap().unwrap().clone()) {
            Some(llvm_type) => llvm_type.types,
            None => panic!("Failed to find type! {} from {}", function.return_type.as_ref().unwrap().name(),
                           display(&llvm_types.keys().map(|key| key.structure.name.clone()).collect(), ", "))
        };
        if found_type.is_struct_type() {
            params.push(found_type.ptr_type(AddressSpace::default()).as_type_ref());
        } else {
            return_type = found_type.as_type_ref();
        }
    }

    for field in &function.fields {
        params.push(match llvm_types.get(field.field_type.unwrap()) {
            Some(llvm_type) => llvm_type.types,
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