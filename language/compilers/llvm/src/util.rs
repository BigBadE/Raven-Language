use std::sync::Arc;
use inkwell::AddressSpace;
use inkwell::types::{BasicType, StructType};
use inkwell::values::FunctionValue;
use syntax::function::Function;
use syntax::r#struct::Struct;
use crate::type_getter::CompilerTypeGetter;

pub fn print_formatted(input: String) {
    let mut output = String::new();
    let mut special = false;
    for char in input.chars() {
        if char == '\\' {
            if special {
                output.push('\\');
            }
            special = !special;
        } else if special {
            if char == 'n' {
                output.push('\n');
            } else {
                output.push(char);
            }
            special = false;
        } else {
            output.push(char);
        }
    }
    println!("{}", output);
}

pub fn create_function_value<'ctx>(function: &Arc<Function>, type_getter: &mut CompilerTypeGetter<'ctx>) -> FunctionValue<'ctx> {
    let mut params = Vec::new();

    for param in &function.fields {
        params.push(From::from(type_getter.get_type(&param.field.field_type)));
    }

    let llvm_function = match &function.return_type {
        Some(returning) => {
            let types = type_getter.get_type(&returning);
            //Structs deallocate their memory when the function ends, so instead the parent function passes a pointer to it.
            if types.is_struct_type() {
                params.insert(0, From::from(types.ptr_type(AddressSpace::default())));
                type_getter.compiler.context.void_type().fn_type(params.as_slice(), false)
            } else {
                types.fn_type(params.as_slice(), false)
            }
        },
        None => type_getter.compiler.context.void_type().fn_type(params.as_slice(), false)
    };

    return type_getter.compiler.module.add_function(&function.name, llvm_function, None);
}

pub async fn create_struct_value<'ctx>(_structure: &Struct, _type_getter: &CompilerTypeGetter<'ctx>) -> StructType<'ctx> {
    todo!()
}