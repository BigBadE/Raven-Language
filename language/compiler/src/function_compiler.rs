use std::ops::Deref;
use inkwell::basic_block::BasicBlock;
use inkwell::types::{AsTypeRef, FunctionType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::code::{Effect, Effects, ExpressionType};
use ast::function::{CodeBody, Function};
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
    if is_modifier(function.modifiers, Modifier::Internal) {
        compile_internal(compiler, &function.name, &function.fields, value);
    } else if is_modifier(function.modifiers, Modifier::Extern) {
        todo!()
    } else {
        let mut function_types = compiler.type_manager.clone();
        compile_block(&function.code, value, &mut function_types, &compiler, &mut 0);

        if !function.code.is_return() {
            match &function.return_type {
                Some(return_type) => panic!("Missing return ({}) for function {}", return_type, function.name),
                None => compiler.builder.build_return(None)
            };
        }
    }
}

pub fn compile_block<'ctx>(code: &CodeBody, function: FunctionValue<'ctx>, variables: &mut CompilerTypeResolver<'ctx>,
                           compiler: &Compiler<'ctx>, id: &mut u64) -> (Option<BasicValueEnum<'ctx>>, BasicBlock<'ctx>) {
    let block = compiler.context.append_basic_block(function, id.to_string().as_str());
    *id += 1;
    compiler.builder.position_at_end(block);
    for line in &code.expressions {
        match line.expression_type {
            ExpressionType::Return => {
                match &line.effect {
                    Effects::NOP() => {}
                    _ => {
                        compiler.builder.build_return(Some(&compile_effect(compiler, function, variables, &line.effect, id).unwrap()));
                    }
                }
            }
            ExpressionType::Line => {
                compile_effect(compiler, function, variables, &line.effect, id);
            }
            ExpressionType::Break => todo!()
        }
    }

    return block;
}

pub fn compile_effect<'ctx>(compiler: &Compiler<'ctx>, function: FunctionValue<'ctx>, variables: &mut CompilerTypeResolver<'ctx>,
                            effect: &Effects, id: &mut u64) -> Option<BasicValueEnum<'ctx>> {
    return match effect {
        Effects::NOP() => panic!("Tried to compile a NOP"),
        Effects::Wrapped(effect) => compile_effect(compiler, function, variables, effect, id),
        Effects::IntegerEffect(effect) =>
            Some(compiler.context.i64_type().const_int(effect.number as u64, false).as_basic_value_enum()),
        Effects::FloatEffect(effect) =>
            Some(compiler.context.f64_type().const_float(effect.number).as_basic_value_enum()),
        Effects::MethodCall(effect) => {
            let mut arguments = Vec::new();

            for argument in &effect.arguments.arguments {
                arguments.push(From::from(compile_effect(compiler, function, variables, argument, id).unwrap()))
            }

            Some(compiler.builder.build_call(compiler.type_manager.functions.get(&effect.method).unwrap().1.unwrap(),
                                        arguments.as_slice(), effect.method.as_str()).try_as_basic_value().left().unwrap())
        }
        Effects::CodeBody(effect) => {
            let block = compile_block(effect, function, &mut variables.clone(), compiler, id);
            compiler.builder.build_unconditional_branch(block);
            None
        },
        Effects::IfStatement(effect) => {
            let mut blocks = Vec::new();

            blocks.push(compile_block(&effect.body, function, &mut variables.clone(), compiler, id));
            compiler.builder.build_conditional_branch()
            for block in effect.else_ifs {
                blocks.push(compile_block(&effect.body, function, &mut variables.clone(), compiler, id));
            }

            if effect.else_body.is_some() {
                blocks.push(compile_block(&effect.else_body.unwrap(), function, &mut variables.clone(), compiler, id));
            }

            return match effect.return_type(&compiler.type_manager) {
                Some(return_type) => {
                    let phi = compiler.builder.build_phi(&compiler.get_llvm_type(return_type), "wtf?");
                    phi.add_incoming(blocks.as_slice());
                    Some(phi)
                }
                None => None
            }
        }
        Effects::OperatorEffect(effect) => {
            let mut arguments = Vec::new();
            if effect.lhs.is_some() {
                let lhs = effect.lhs.as_ref().unwrap();
                arguments.push(From::from(compile_effect(compiler, function, variables, lhs, id).unwrap()));
            }
            if effect.rhs.is_some() {
                let rhs = effect.rhs.as_ref().unwrap();
                arguments.push(From::from(compile_effect(compiler, function, variables, rhs, id).unwrap()));
            }

            Some(compiler.builder.build_call(compiler.type_manager.functions.get(&effect.operator).unwrap().1.unwrap(),
                                        arguments.as_slice(), effect.operator.as_str()).try_as_basic_value().left().unwrap())
        }
        Effects::VariableLoad(effect) =>
            Some(variables.get(&effect.name).expect(format!("Unknown variable called {}", effect.name).as_str()).clone()),
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
            let value = compile_effect(compiler, function, variables, &variable.effect, id).unwrap();

            variables.variables.insert(variable.variable.clone(), value);
            compiler.builder.build_store(pointer, value);
            Some(value)
        }
    };
}