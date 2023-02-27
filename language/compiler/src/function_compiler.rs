use std::collections::HashMap;
use inkwell::types::{AsTypeRef, FunctionType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::code::Effects;
use ast::function::Function;
use crate::compiler::Compiler;
use crate::instructions::math::math_operation;

pub fn compile_function<'ctx>(function: &Function, compiler: &Compiler<'ctx>) -> FunctionValue<'ctx> {
    let return_type = match &function.return_type {
        Some(found) => compiler.get_type(&found.value).as_type_ref(),
        None => compiler.context.void_type().as_type_ref()
    };

    let mut params: Vec<LLVMTypeRef> = function.fields.iter().map(|field| compiler.get_type(&field.field_type.value).as_type_ref()).collect();

    let fn_type = unsafe {
        FunctionType::new(
            LLVMFunctionType(return_type, params.as_mut_ptr(), params.len() as u32, false as i32))
    };

    let fn_value = compiler.module.add_function(function.name.value.as_str(), fn_type, None);

    let block = compiler.context.append_basic_block(fn_value, "entry");
    compiler.builder.position_at_end(block);
    compile_block(function, &mut HashMap::new(), &compiler);

    if !function.code.expressions.last().map_or(false, |expression| expression.effect.unwrap().is_return()) {
        match &function.return_type {
            Some(return_type) => panic!("Missing return type {} for function {}", return_type, function.name),
            None => compiler.builder.build_return(None)
        };
    }
    return fn_value;
}

pub fn compile_block<'ctx>(function: &Function, variables: &mut HashMap<String, BasicValueEnum<'ctx>>, compiler: &Compiler<'ctx>) {
    for line in &function.code.expressions {
        compile_effect(function, compiler, variables, &line.effect);
    }
}

pub fn compile_effect<'ctx>(function: &Function, compiler: &Compiler<'ctx>,
                            variables: &mut HashMap<String, BasicValueEnum<'ctx>>, effect: &Effects) -> BasicValueEnum<'ctx> {
    return match effect {
        Effects::IntegerEffect(effect) =>
            compiler.context.i64_type().const_int(effect.number, true).as_basic_value_enum(),
        Effects::FloatEffect(effect) =>
            compiler.context.f64_type().const_float(effect.number).as_basic_value_enum(),
        Effects::MethodCall(_effect) => panic!("Method calls not implemented yet!"),
        Effects::VariableLoad(effect) =>
            variables.get(&effect.name.value).expect(format!("Unknown variable called {}", effect.name.value).as_str()).clone(),
        Effects::ReturnEffect(effect) => {
            let value = compile_effect(function, compiler, variables, &effect.effect);
            compiler.builder.build_return(Some(&value));

            value
        }
        Effects::MathEffect(effect) => {
            let value = match &effect.target {
                Some(effect) => Some(compile_effect(function, compiler, variables, effect)),
                None => None
            };
            math_operation(effect.operator, compiler, value,
                                            compile_effect(function, compiler, variables, &effect.effect))
        }
    };
}