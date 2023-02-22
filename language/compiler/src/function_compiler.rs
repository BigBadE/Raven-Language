use std::collections::HashMap;
use inkwell::types::{AsTypeRef, FunctionType, IntType};
use inkwell::values::{BasicValue, FunctionValue, IntValue};
use inkwell::values::AnyValueEnum::IntValue;
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::code::{Effects, MathOperator};
use ast::function::Function;
use crate::compiler::Compiler;
use crate::types::{Type, TypeManager, Value};

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

    if !function.code.expressions.last().map_or(false, |expression| expression.effect.unwrap().is_return()) {
        match &function.return_type {
            Some(return_type) => panic!("Missing return type {} for function {}", return_type, function.name),
            None => compiler.builder.build_return(None)
        };
    }
    return fn_value;
}

pub fn compile_block<'ctx>(function: &Function, variables: &HashMap<String, Box<dyn BasicValue<'ctx>>>, compiler: &Compiler<'ctx>) {
    for line in &function.code.expressions {
        compile_effect(function, compiler, variables, &line.effect);
    }
}

pub fn compile_effect<'ctx>(function: &Function, compiler: &Compiler<'ctx>, types: TypeManager,
                            variables: &HashMap<String, Value<'ctx>>, effect: &Effects) -> Value<'ctx> {
    return match effect {
        Effects::IntegerEffect(effect) =>
            Value::new(types.get_type("i64").unwrap(),
                       compiler.context.i64_type().const_int(effect.number, true)),
        Effects::FloatEffect(effect) =>
            Value::new(types.get_type("f64").unwrap(),
                       compiler.context.f64_type().const_float(effect.number)),
        Effects::MethodCall(effect) => panic!("Method calls not implemented yet!"),
        Effects::VariableLoad(effect) =>
            variables.get(&effect.name.value).expect("Unknown variable called " + effect.name.value),
        Effects::ReturnEffect(effect) =>
            Value::new(types.get_type("void").unwrap(),
                       compiler.builder.build_return(Some(compile_effect(function, compiler, types, variables, &effect.effect).as_ref()))),
        Effects::MathEffect(effect) =>
    };
}