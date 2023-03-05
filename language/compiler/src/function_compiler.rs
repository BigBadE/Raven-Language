use std::ops::Deref;
use inkwell::types::{AsTypeRef, FunctionType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::code::{Effects, ExpressionType};
use ast::function::Function;
use ast::{is_modifier, Modifier};
use ast::type_resolver::TypeResolver;
use crate::compiler::Compiler;
use crate::instructions::compile_internal;
use crate::types::type_resolver::CompilerTypeResolver;

pub fn get_function_value<'ctx>(function: &Function, compiler: &Compiler<'ctx>) -> FunctionValue<'ctx> {
    let return_type = match &function.return_type {
        Some(found) => compiler.get_llvm_type(found).as_type_ref(),
        None => compiler.context.void_type().as_type_ref()
    };

    let mut params: Vec<LLVMTypeRef> = function.fields.iter().map(
        |field| compiler.get_llvm_type(&field.field_type).as_type_ref()).collect();

    let fn_type = unsafe {
        FunctionType::new(LLVMFunctionType(return_type, params.as_mut_ptr(),
                                           params.len() as u32, false as i32))
    };

    return compiler.module.add_function(function.name.as_str(), fn_type, None);
}

pub fn compile_function<'ctx>(function: &Function, compiler: &Compiler<'ctx>) {
    let value = compiler.type_manager.functions.get(&function.name).unwrap().1.unwrap();
    let block = compiler.context.append_basic_block(value, "entry");
    compiler.builder.position_at_end(block);
    if is_modifier(function.modifiers, Modifier::Internal) {
        compile_internal(compiler, &function.name, &function.fields, value);
    } else if is_modifier(function.modifiers, Modifier::Extern) {
        todo!()
    } else {
        let mut function_types = compiler.type_manager.clone();
        compile_block(function, &mut function_types, &compiler);

        if !function.code.is_return() {
            match &function.return_type {
                Some(return_type) => panic!("Missing return ({}) for function {}", return_type, function.name),
                None => compiler.builder.build_return(None)
            };
        }
    }
}

pub fn compile_block<'ctx>(function: &Function, variables: &mut CompilerTypeResolver<'ctx>,
                           compiler: &Compiler<'ctx>) {
    for line in &function.code.expressions {
        match line.expression_type {
            ExpressionType::Return => {
                match &line.effect {
                    Effects::NOP() => {}
                    _ => {
                        compiler.builder.build_return(Some(&compile_effect(compiler, variables, &line.effect)));
                    }
                }
            }
            ExpressionType::Line => {
                compile_effect(compiler, variables, &line.effect);
            }
            ExpressionType::Break => todo!()
        }
    }
}

pub fn compile_effect<'ctx>(compiler: &Compiler<'ctx>, variables: &mut CompilerTypeResolver<'ctx>,
                            effect: &Effects) -> BasicValueEnum<'ctx> {
    return match effect {
        Effects::NOP() => panic!("Tried to compile a NOP"),
        Effects::Wrapped(effect) => compile_effect(compiler, variables, effect),
        Effects::IntegerEffect(effect) =>
            compiler.context.i64_type().const_int(effect.number as u64, false).as_basic_value_enum(),
        Effects::FloatEffect(effect) =>
            compiler.context.f64_type().const_float(effect.number).as_basic_value_enum(),
        Effects::MethodCall(effect) => {
            let mut arguments = Vec::new();

            for argument in &effect.arguments.arguments {
                arguments.push(From::from(compile_effect(compiler, variables, argument)))
            }

            compiler.builder.build_call(compiler.type_manager.functions.get(&effect.method).unwrap().1.unwrap(),
                                              arguments.as_slice(), effect.method.as_str()).try_as_basic_value().left().unwrap()
        }
        Effects::OperatorEffect(effect) => {
            let mut arguments = Vec::new();
            if effect.lhs.is_some() {
                let lhs = effect.lhs.as_ref().unwrap();
                arguments.push(From::from(compile_effect(compiler, variables, lhs)));
            }
            if effect.rhs.is_some() {
                let rhs = effect.rhs.as_ref().unwrap();
                arguments.push(From::from(compile_effect(compiler, variables, rhs)));
            }

            compiler.builder.build_call(compiler.type_manager.functions.get(&effect.operator).unwrap().1.unwrap(),
                                              arguments.as_slice(), effect.operator.as_str()).try_as_basic_value().left().unwrap()
        }
        Effects::VariableLoad(effect) =>
            variables.get(&effect.name).expect(format!("Unknown variable called {}", effect.name).as_str()).clone(),
        Effects::AssignVariable(variable) => {
            let pointer = compiler.builder.build_alloca(*compiler.get_llvm_type(
                match variable.effect.unwrap().return_type(variables) {
                    Some(found_type) => found_type,
                    None => match &variable.given_type {
                        Some(found_type) => compiler.type_manager.get_type(found_type).unwrap(),
                        None => panic!("Unable to find type for variable {}
                    (assign it using a let statement to specify the type)", variable.variable)
                    }
                }.deref()), variable.variable.as_str());
            let value = compile_effect(compiler, variables, &variable.effect);

            variables.variables.insert(variable.variable.clone(), value);
            compiler.builder.build_store(pointer, value);
            value
        }
    };
}