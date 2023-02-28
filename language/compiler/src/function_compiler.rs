use either::Either;
use inkwell::types::{AsTypeRef, FunctionType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use llvm_sys::core::LLVMFunctionType;
use llvm_sys::prelude::LLVMTypeRef;
use ast::code::{Effects, ExpressionType};
use ast::function::Function;
use crate::compiler::Compiler;
use crate::instructions::math::math_operation;
use crate::types::type_resolver::FunctionTypeResolver;

pub fn get_function_value<'ctx>(function: &Function, compiler: &Compiler<'ctx>) -> FunctionValue<'ctx> {
    let return_type = match &function.return_type {
        Some(found) => compiler.get_type(&found).as_type_ref(),
        None => compiler.context.void_type().as_type_ref()
    };

    let mut params: Vec<LLVMTypeRef> = function.fields.iter().map(|field| compiler.get_type(&field.field_type).as_type_ref()).collect();

    let fn_type = unsafe {
        FunctionType::new(
            LLVMFunctionType(return_type, params.as_mut_ptr(), params.len() as u32, false as i32))
    };

    return compiler.module.add_function(function.name.as_str(), fn_type, None);
}

pub fn compile_function<'ctx>(function: &Function, compiler: &Compiler<'ctx>) {
    let block = compiler.context.append_basic_block(compiler.functions.get(&function.name).unwrap().1, "entry");
    compiler.builder.position_at_end(block);
    compile_block(function, &mut FunctionTypeResolver::new(&compiler), &compiler);

    if !function.code.is_return() {
        match &function.return_type {
            Some(return_type) => panic!("Missing return ({}) for function {}", return_type, function.name),
            None => compiler.builder.build_return(None)
        };
    }
}

pub fn compile_block<'com, 'ctx>(function: &Function, variables: &mut FunctionTypeResolver<'com, 'ctx>,
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
            },
            ExpressionType::Line => {
                compile_effect(compiler, variables, &line.effect);
            },
            ExpressionType::Break => todo!()
        }
    }
}

pub fn compile_effect<'com, 'ctx>(compiler: &Compiler<'ctx>, variables: &mut FunctionTypeResolver<'com, 'ctx>,
                            effect: &Effects) -> BasicValueEnum<'ctx> {
    return match effect {
        Effects::NOP() => panic!("Tried to compile a NOP"),
        Effects::IntegerEffect(effect) =>
            compiler.context.i64_type().const_int(effect.number as u64, false).as_basic_value_enum(),
        Effects::FloatEffect(effect) =>
            compiler.context.f64_type().const_float(effect.number).as_basic_value_enum(),
        Effects::MethodCall(effect) => {
            let arguments = Vec::new();
            //TODO args
            match compiler.builder.build_call(compiler.functions.get(&effect.method).unwrap().1,
                                        arguments.as_slice(), effect.method.as_str()).try_as_basic_value() {
                Either::Left(value) => value,
                //TODO figure this bs out
                Either::Right(_) => compiler.context.i64_type().const_int(0xDEADBEEF, false).as_basic_value_enum()
            }
        },
        Effects::VariableLoad(effect) =>
            variables.get(&effect.name).expect(format!("Unknown variable called {}", effect.name).as_str()).clone(),
        Effects::MathEffect(effect) => {
            let value = match &effect.target {
                Some(effect) => Some(compile_effect(compiler, variables, effect)),
                None => None
            };
            math_operation(effect.operator, compiler, value,
                           compile_effect(compiler, variables, &effect.effect))
        }
        Effects::AssignVariable(variable) => {
            let pointer = compiler.builder.build_alloca(*match variable.effect.unwrap().return_type(variables) {
                Some(found_type) => compiler.types.get_type_err(found_type.as_str()),
                None => match &variable.given_type {
                    Some(found_type) => compiler.types.get_type_err(found_type.as_str()),
                    None => panic!("Unable to find type for variable {}
                    (assign it using a let statement to specify the type)", variable.variable)
                }
            }, variable.variable.as_str());
            let value = compile_effect(compiler, variables, &variable.effect);
            variables.variables.insert(variable.variable.clone(), value);
            compiler.builder.build_store(pointer, value);
            value
        }
    };
}