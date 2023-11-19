use crate::type_getter::CompilerTypeGetter;
use inkwell::module::Linkage;
use inkwell::types::BasicType;
use inkwell::values::FunctionValue;
use std::ops::Deref;
use std::sync::Arc;
use syntax::function::CodelessFinalizedFunction;
use syntax::types::FinalizedTypes;

pub fn print_formatted(input: String) {
    let mut output = String::default();
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

pub fn create_function_value<'ctx>(
    function: &Arc<CodelessFinalizedFunction>,
    type_getter: &mut CompilerTypeGetter<'ctx>,
    linkage: Option<Linkage>,
) -> FunctionValue<'ctx> {
    let mut params = Vec::default();

    for param in &function.arguments {
        params.push(From::from(type_getter.get_type(&param.field.field_type)));
    }

    let llvm_function = match &function.return_type {
        Some(returning) => {
            let mut returning = returning;
            if let FinalizedTypes::Reference(inner) = returning {
                returning = inner.deref();
            }
            let types =
                type_getter.get_type(&FinalizedTypes::Reference(Box::new(returning.clone())));
            //Structs deallocate their memory when the function ends, so instead the parent function passes a pointer to it.
            //TODO not used for now cause malloc is used, but for future speed ups will be needed
            /*if types.is_struct_type() {
                params.insert(0, From::from(types.ptr_type(AddressSpace::default())));
                type_getter.compiler.context.void_type().fn_type(params.as_slice(), false)
            } else {*/
            types.fn_type(params.as_slice(), false)
            //}
        }
        None => type_getter
            .compiler
            .context
            .void_type()
            .fn_type(params.as_slice(), false),
    };

    return type_getter
        .compiler
        .module
        .add_function(&function.data.name, llvm_function, linkage);
}
