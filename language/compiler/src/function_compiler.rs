use inkwell::types::{AsTypeRef, FunctionType};
use inkwell::values::FunctionValue;
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::function::Function;
use crate::compiler::Compiler;

pub fn compile_function<'ctx>(function: &Function, compiler: &Compiler<'ctx>) -> FunctionValue<'ctx> {
    let return_type = match &function.return_type {
        Some(found) => *compiler.get_type(&found.value),
        None => compiler.context.void_type().as_type_ref()
    };

    let mut params: Vec<LLVMTypeRef> = function.fields.iter().map(|field| *compiler.get_type(&field.field_type.value)).collect();

    let fn_type = unsafe { FunctionType::new(
        LLVMFunctionType(return_type, params.as_mut_ptr(), params.len() as u32, false as i32)) };

    let fn_value = compiler.module.add_function(function.name.value.as_str(), fn_type, None);

    let block = compiler.context.append_basic_block(fn_value, "entry");
    compiler.builder.position_at_end(block);

    compiler.builder.build_return(None);
    return fn_value;
}