use std::mem::MaybeUninit;
use std::ops::Deref;
use inkwell::basic_block::BasicBlock;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use ast::code::{Effect, Effects, ExpressionType};
use ast::function::{CodeBody, Function};
use ast::{is_modifier, Modifier};
use crate::compiler::Compiler;
use crate::internal::instructions::compile_internal;
use crate::types::type_resolver::CompilerTypeResolver;
use crate::util::print_formatted;

pub fn compile_function<'ctx>(function: &Function, compiler: &Compiler<'ctx>, type_manager: &CompilerTypeResolver<'ctx>) {
    let value = type_manager.functions.get(&function.name).unwrap().1;
    if is_modifier(function.modifiers, Modifier::Internal) {
        compile_internal(compiler, &function.name, &function.fields, value);
    } else if is_modifier(function.modifiers, Modifier::Extern) {
        todo!()
    } else {
        let mut function_types = type_manager.clone();
        compile_block(&function.code, value, &mut function_types, &compiler, &mut 0);
        println!("Func:");
        print_formatted(value.to_string());
        compiler.builder.build_return(None);

        if !function.code.is_return() {
            match &function.return_type {
                Some(return_type) => panic!("Missing return ({}) for function {}", return_type.unwrap(), function.name),
                None => compiler.builder.build_return(None)
            };
        }
    }
}

pub fn compile_block<'ctx>(code: &CodeBody, function: FunctionValue<'ctx>,
                           variables: &mut CompilerTypeResolver<'ctx>, compiler: &Compiler<'ctx>, id: &mut u64) -> BasicBlock<'ctx> {
    let mut block = compiler.context.append_basic_block(function, id.to_string().as_str());
    *id += 1;
    compiler.builder.position_at_end(block);
    for line in &code.expressions {
        match line.expression_type {
            ExpressionType::Return => {
                match &line.effect {
                    Effects::NOP() => {}
                    _ => {
                        compiler.builder.build_return(Some(&
                            compile_effect(compiler, &mut block, function, variables, &line.effect, id).unwrap()));
                    }
                }
            }
            ExpressionType::Line => {
                compile_effect(compiler, &mut block, function, variables, &line.effect, id);
            }
            ExpressionType::Break => todo!()
        }
    }

    return block;
}

pub fn compile_effect<'ctx>(compiler: &Compiler<'ctx>, block: &mut BasicBlock<'ctx>, function: FunctionValue<'ctx>,
                            variables: &mut CompilerTypeResolver<'ctx>, effect: &Effects, id: &mut u64) -> Option<BasicValueEnum<'ctx>> {
    return match effect {
        Effects::NOP() => panic!("Tried to compile a NOP"),
        Effects::Wrapped(effect) => compile_effect(compiler, block, function, variables, effect, id),
        Effects::IntegerEffect(effect) =>
            Some(compiler.context.i64_type().const_int(effect.number as u64, false).as_basic_value_enum()),
        Effects::FloatEffect(effect) =>
            Some(compiler.context.f64_type().const_float(effect.number).as_basic_value_enum()),
        Effects::MethodCall(effect) => {
            let mut arguments = Vec::new();

            for argument in &effect.arguments.arguments {
                arguments.push(From::from(compile_effect(compiler, block, function, variables, argument, id).unwrap()))
            }

            *id += 1;
            Some(compiler.builder.build_call(variables.functions.get(&effect.method).unwrap().1,
                                             arguments.as_slice(), &(*id - 1).to_string()).try_as_basic_value().left().unwrap())
        },
        Effects::FieldLoad(effect) => {
            let from = compile_effect(compiler, block, function, variables, &effect.calling, id).unwrap();
            *id += 1;
            let mut offset = 0;
            for field in effect.calling.unwrap().return_type().unwrap().unwrap().get_fields() {
                if field.field.name != effect.name {
                    offset += field.field.field_type.unwrap().size;
                }
            }
            Some(compiler.builder.build_extract_value(from.into_struct_value(), offset, &(*id - 1).to_string()).unwrap())
        }
        Effects::CreateStruct(effect) => {
            let types = effect.structure.unwrap();

            let structure = variables.llvm_types.get(types).unwrap()
                .into_struct_type().get_undef();

            let mut arguments = vec![MaybeUninit::uninit(); effect.parsed_effects.as_ref().unwrap().len()];

            for (index, effect) in effect.parsed_effects.as_ref().unwrap() {
                let returned = compile_effect(compiler, block, function, variables, effect, id).unwrap();
                arguments.insert(*index, MaybeUninit::new((returned,
                                                           effect.unwrap().return_type().unwrap().unwrap().size)));
            }

            let mut offset = 0;
            for argument in arguments {
                let (effect, size) = unsafe { argument.assume_init() };
                compiler.builder.build_insert_value(structure, effect,
                                                    offset, id.to_string().as_str());
                offset += size;
                *id += 1;
            }

            Some(structure.as_basic_value_enum())
        }
        Effects::CodeBody(effect) => {
            //Start block
            let start = compile_block(effect, function, &mut variables.clone(), compiler, id);
            compiler.builder.position_at_end(*block);
            compiler.builder.build_unconditional_branch(start);
            compiler.builder.position_at_end(start);

            //End block
            let end = compiler.context.append_basic_block(function, id.to_string().as_str());
            *id += 1;
            compiler.builder.build_unconditional_branch(end);
            compiler.builder.position_at_end(end);
            *block = end;
            None
        }
        Effects::IfStatement(effect) => {
            //Compile the if body
            let then = compile_block(&effect.body, function, variables, compiler, id);

            //Add all the else ifs, and finally the else, to else_ifs
            let mut else_ifs = Vec::new();
            for (value, effect) in &effect.else_ifs {
                else_ifs.push((Some(value), Some(effect)));
            }
            if effect.else_body.is_some() {
                else_ifs.push((Some(&effect.else_body.as_ref().unwrap()), None));
            } else {
                else_ifs.push((None, None));
            }

            //Recursively compile the else ifs
            let (other, end) = compile_elseifs(&else_ifs, 0, function, variables, compiler, id);

            compiler.builder.position_at_end(then);
            compiler.builder.build_unconditional_branch(end);

            //Go back to the start of the if
            compiler.builder.position_at_end(*block);
            compiler.builder.build_conditional_branch(
                compile_effect(compiler, block, function, variables, &effect.condition, id).unwrap().into_int_value(),
                then, other);

            compiler.builder.position_at_end(end);
            *block = end;

            //If it's an if with a return value, use phi to get the value
            return match effect.return_type() {
                Some(_return_type) => {
                    todo!();
                    /*let phi = compiler.builder.build_phi(
                        *compiler.type_manager.llvm_types.get(return_type.unwrap()).unwrap(), &id.to_string());
                    *id += 1;
                    phi.add_incoming(blocks.as_slice());
                    Some(phi.as_basic_value())*/
                }
                None => None
            };
        }
        Effects::OperatorEffect(effect) => {
            let mut arguments = Vec::new();
            for argument in &effect.effects {
                arguments.push(From::from(compile_effect(compiler, block, function, variables, argument, id).unwrap()));
            }

            *id += 1;
            Some(compiler.builder.build_call(variables.functions.get(effect.function.as_ref().unwrap()).unwrap().1,
                                             arguments.as_slice(), &(*id - 1).to_string()).try_as_basic_value().left().unwrap())
        }
        Effects::VariableLoad(effect) =>
            Some(variables.variables.get(&effect.name).expect(format!("Unknown variable called {}", effect.name).as_str()).clone()),
        Effects::AssignVariable(variable) => {
            let pointer = compiler.builder.build_alloca(*variables.llvm_types.get(
                match variable.effect.unwrap().return_type() {
                    Some(found_type) => found_type.unwrap().clone(),
                    None => match &variable.effect.unwrap().return_type() {
                        Some(found_type) => found_type.unwrap().clone(),
                        None => panic!("Unable to find type for variable {}
                    (assign it using a let statement to specify the type)", variable.variable)
                    }
                }.deref()).unwrap(), variable.variable.as_str());
            let value = compile_effect(compiler, block, function, variables, &variable.effect, id).unwrap();

            variables.variables.insert(variable.variable.clone(), value);
            compiler.builder.build_store(pointer, value);
            Some(value)
        }
    };
}

//LLVM dies if the last instruction isn't a return, so this is weirdly structured
//to make sure the end is created last.
fn compile_elseifs<'ctx>(effects: &Vec<(Option<&CodeBody>, Option<&Effects>)>, index: usize, function: FunctionValue<'ctx>,
                   variables: &mut CompilerTypeResolver<'ctx>, compiler: &Compiler<'ctx>, id: &mut u64) -> (BasicBlock<'ctx>, BasicBlock<'ctx>) {
    if effects.len() == 1 {
        return match effects.get(0).unwrap().0 {
            Some(effect) => {
                let block = compile_block(effect, function, variables, compiler, id);
                let end = compiler.context.append_basic_block(function, &id.to_string());
                *id += 1;
                compiler.builder.build_unconditional_branch(end);
                (block, end)
            },
            None => {
                let end = compiler.context.append_basic_block(function, &id.to_string());
                *id += 1;
                (end, end)
            }
        }
    }

    let mut new_block = compiler.context.append_basic_block(function, &id.to_string());
    compiler.builder.position_at_end(new_block);
    *id += 1;

    let (other, end);
    if index < effects.len() - 2 {
        let tuple = compile_elseifs(&effects, index + 1, function, variables, compiler, id);
        other = tuple.0;
        end = tuple.1;
    } else {
        other = compile_block(effects.get(index+1).unwrap().0.unwrap(), function, variables, compiler, id);
        compiler.builder.position_at_end(other);
        end = compiler.context.append_basic_block(function, &id.to_string());
        *id += 1;
        compiler.builder.build_unconditional_branch(end);
    }

    let (body, effect) = effects.get(index).unwrap();
    let then = compile_block(body.unwrap(), function, variables, compiler, id);
    compiler.builder.build_unconditional_branch(end);

    compiler.builder.position_at_end(new_block);
    let comparison = compile_effect(compiler, &mut new_block, function, variables, &effect.unwrap(), id)
        .unwrap().into_int_value();
    compiler.builder.build_conditional_branch(comparison, then, other);

    return (new_block, end);
}