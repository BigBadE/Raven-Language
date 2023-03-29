use std::mem;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::rc::Rc;
use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::types::BasicType;
use inkwell::values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue};
use ast::code::{Effect, Effects, Expression, ExpressionType, MethodCall};
use ast::function::{Arguments, CodeBody, display, Function};
use ast::{is_modifier, Modifier};
use ast::blocks::IfStatement;
use crate::compiler::Compiler;
use crate::internal::instructions::compile_internal;
use crate::types::type_resolver::{CompilerTypeResolver, get_func_value};
use crate::util::print_formatted;

pub fn compile_function<'a, 'ctx>(function: &Function, compiler: &Compiler<'ctx>, type_manager: &CompilerTypeResolver<'a, 'ctx>) {
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

pub fn compile_block<'a, 'ctx>(code: &CodeBody, function: FunctionValue<'ctx>,
                           variables: &mut CompilerTypeResolver<'a, 'ctx>, compiler: &Compiler<'ctx>, id: &mut u64) -> BasicBlock<'ctx> {
    let mut block = compiler.context.append_basic_block(function, id.to_string().as_str());
    *id += 1;
    compiler.builder.position_at_end(block);
    for line in &code.expressions {
        match line.expression_type {
            ExpressionType::Return => {
                match &line.effect {
                    Effects::NOP() => {}
                    _ => {
                        let mut returned = compile_effect(compiler, &mut block, function, variables, &line.effect, id).unwrap();

                        let func = &variables.functions.get(function.get_name().to_str().unwrap()).unwrap().0;

                        let returning = variables.get_llvm_type(func.return_type.as_ref().unwrap().unwrap());

                        if &returned.get_type() != returning {
                            let pointer = compiler.builder.build_pointer_cast(
                                returned.into_pointer_value(), returning.ptr_type(AddressSpace::default()),
                                &id.to_string());
                            *id += 1;
                            returned = compiler.builder.build_load(pointer, &id.to_string()).as_basic_value_enum();
                            *id += 1;
                        }

                        if returned.is_struct_value() {
                            compiler.builder.build_store(function.get_first_param().unwrap().into_pointer_value(),
                                                         returned);
                            compiler.builder.build_return(None);
                        } else if returned.is_pointer_value() &&
                            returned.into_pointer_value().get_type().get_element_type().is_struct_type() {
                            compiler.builder.build_store(function.get_first_param().unwrap().into_pointer_value(),
                                                         compiler.builder.build_load(returned.into_pointer_value(), &id.to_string()));
                            *id += 1;
                            compiler.builder.build_return(None);
                        } else {
                            compiler.builder.build_return(Some(&returned));
                        }
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

pub fn compile_effect<'a, 'ctx>(compiler: &Compiler<'ctx>, block: &mut BasicBlock<'ctx>, function: FunctionValue<'ctx>,
                            variables: &mut CompilerTypeResolver<'a, 'ctx>, effect: &Effects, id: &mut u64) -> Option<BasicValueEnum<'ctx>> {
    return match effect {
        Effects::NOP() => panic!("Tried to compile a NOP"),
        Effects::Wrapped(effect) => compile_effect(compiler, block, function, variables, effect, id),
        Effects::IntegerEffect(effect) =>
            Some(compiler.context.i64_type().const_int(effect.number as u64, false).as_basic_value_enum()),
        Effects::FloatEffect(effect) =>
            Some(compiler.context.f64_type().const_float(effect.number).as_basic_value_enum()),
        Effects::MethodCall(effect) => {
            let mut arguments = Vec::new();

            let calling = variables.functions.get(&effect.method).unwrap().1;
            if effect.return_type().is_some() && !calling.get_type().get_return_type().is_some() {
                let pointer = compiler.builder.build_alloca(
                    variables.llvm_types.get(effect.return_type().unwrap().unwrap()).unwrap().types, &id.to_string());
                *id += 1;
                arguments.push(BasicMetadataValueEnum::from(pointer.as_basic_value_enum()));

                if effect.calling.is_some() {
                    arguments.push(From::from(compile_effect(compiler, block, calling, variables, effect.calling.as_ref().unwrap(), id).unwrap()));
                }

                for argument in &effect.arguments.arguments {
                    arguments.push(From::from(compile_effect(compiler, block, calling, variables, argument, id).unwrap()))
                }

                *id += 1;
                compiler.builder.build_call(calling, arguments.as_slice(), &(*id - 1).to_string());
                Some(pointer.as_basic_value_enum())
            } else {
                if effect.calling.is_some() {
                    arguments.push(From::from(compile_effect(compiler, block, calling, variables, effect.calling.as_ref().unwrap(), id).unwrap()));
                }

                for argument in &effect.arguments.arguments {
                    arguments.push(From::from(compile_effect(compiler, block, calling, variables, argument, id).unwrap()))
                }

                *id += 1;
                Some(compiler.builder.build_call(calling, arguments.as_slice(),
                                                 &(*id - 1).to_string()).try_as_basic_value().left().unwrap())
            }
        },
        Effects::ForStatement(effect) => {
            let comparison_block = compiler.context.append_basic_block(function, id.to_string().as_str());
            //Continue to comparison block
            compiler.builder.build_unconditional_branch(comparison_block);
            *id += 1;

            //Set end check
            let name = effect.effect.unwrap().return_type().unwrap().unwrap().structure.functions
                .iter().find(|func: &&String| func.split("::").last().unwrap().contains("is_end")).unwrap().clone();

            let mut comparison = MethodCall::new(Some(effect.effect.clone()), name,
                                                 Arguments::new(vec!()), (0, 0));
            comparison.finalize(variables);

            let mut temp = variables.clone();
            compile_effect(compiler, block, function, &mut temp, &effect.effect, id);
            //Build for body
            let for_body = compile_block(&effect.code_block, function, &mut temp, compiler, id);
            compiler.builder.build_unconditional_branch(for_body);
            //Build end
            let end = compiler.context.append_basic_block(function, id.to_string().as_str());
            *id += 1;

            compiler.builder.position_at_end(comparison_block);
            let comparison = compile_effect(compiler, block, function, variables,
                                            &Effects::MethodCall(Box::new(comparison)), id).unwrap();
            compiler.builder.build_conditional_branch(comparison.into_int_value(), end, for_body);

            *block = end;
            compiler.builder.position_at_end(*block);
            None
        },
        Effects::SwitchStatement(effect) => {
            let mut if_effect: Option<IfStatement> = None;

            for value in &effect.possible {
                if let Some(if_effect) = &mut if_effect {
                    let case = compile_case(&effect.value, value);
                    if_effect.else_ifs.push(case);
                } else {
                    let first = compile_case(&effect.value, value);
                    if_effect = Some(IfStatement::new(first.0, first.1, (0, 0)));
                }
            }

            compile_effect(compiler, block, function, variables, &Effects::IfStatement(Box::new(if_effect.unwrap())), id)
        }
        Effects::FieldLoad(effect) => {
            let from = compile_effect(compiler, block, function, variables, &effect.calling, id).unwrap();
            let mut offset = 1;
            for field in effect.calling.unwrap().return_type().unwrap().unwrap().get_fields() {
                if field.field.name != effect.name {
                    offset += 1;
                } else {
                    break;
                }
            }

            let pointer;
            if !from.is_pointer_value() {
                pointer = compiler.builder.build_alloca(from.get_type(), &id.to_string());
                *id += 1;
                compiler.builder.build_store(pointer, from.into_struct_value().as_basic_value_enum());
            } else {
                pointer = from.into_pointer_value();
            }
            *id += 2;
            Some(compiler.builder.build_load(
                compiler.builder.build_struct_gep(pointer, offset, &(*id - 2).to_string()).unwrap(),
                &(*id - 1).to_string()).as_basic_value_enum())
        }
        Effects::CreateStruct(effect) => {
            let types = effect.structure.unwrap();

            let mut arguments = vec![MaybeUninit::uninit(); effect.parsed_effects.as_ref().unwrap().len()];

            for (index, effect) in effect.parsed_effects.as_ref().unwrap() {
                let returned = compile_effect(compiler, block, function, variables, effect, id).unwrap();
                let found_size = effect.unwrap().return_type().unwrap().unwrap().size;
                *arguments.get_mut(*index).unwrap() = MaybeUninit::new((returned, found_size));
            }

            let structure = variables.llvm_types.get(types).unwrap().types;
            let pointer = compiler.builder.build_alloca(structure, &id.to_string());
            *id += 1;

            let mut offset = 0;
            for argument in arguments {
                let (effect, _size) = unsafe { argument.assume_init() };

                let pointer = compiler.builder.build_struct_gep(pointer, offset, &id.to_string()).unwrap();
                *id += 1;
                compiler.builder.build_store(pointer, effect);
                offset += 1;
            }

            Some(pointer.as_basic_value_enum())
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

            return None;
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
            Some(variables.variables.get(&effect.name).expect(format!("Unknown variable called {}", effect.name).as_str()).1.unwrap().clone()),
        Effects::AssignVariable(variable) => {
            let pointer = compiler.builder.build_alloca(variables.llvm_types.get(
                match variable.effect.unwrap().return_type() {
                    Some(found_type) => found_type.unwrap().clone(),
                    None => match &variable.effect.unwrap().return_type() {
                        Some(found_type) => found_type.unwrap().clone(),
                        None => panic!("Unable to find type for variable {}
                    (assign it using a let statement to specify the type)", variable.variable)
                    }
                }.deref()).unwrap().types, variable.variable.as_str());
            let value = compile_effect(compiler, block, function, variables, &variable.effect, id).unwrap();

            variables.variables.insert(variable.variable.clone(),
                                       (variable.effect.unwrap().return_type().unwrap().unwrap().clone(), Some(value)));
            compiler.builder.build_store(pointer, value);
            Some(value)
        }
    };
}

fn compile_case(value: &Effects, input: &(Effects, Effects)) -> (CodeBody, Effects) {
    let body;
    match &input.1 {
        Effects::CodeBody(box_body) => body = Box::into_inner(box_body.clone()),
        _ => body = CodeBody::new(vec!(Expression::new(ExpressionType::Break, input.1.clone())))
    }
    let method = value.unwrap().return_type().unwrap().unwrap().name.clone() + "::equals";
    let calling = Effects::MethodCall(Box::new(MethodCall::new(Some(value.clone()), method,
    Arguments::new(vec!(input.0.clone())), (0, 0))));
    return (body, calling);
}

//LLVM dies if the last instruction isn't a return, so this is weirdly structured
//to make sure the end is created last.
fn compile_elseifs<'a, 'ctx>(effects: &Vec<(Option<&CodeBody>, Option<&Effects>)>, index: usize, function: FunctionValue<'ctx>,
                         variables: &mut CompilerTypeResolver<'a, 'ctx>, compiler: &Compiler<'ctx>, id: &mut u64) -> (BasicBlock<'ctx>, BasicBlock<'ctx>) {
    if effects.len() == 1 {
        return match effects.get(0).unwrap().0 {
            Some(effect) => {
                let block = compile_block(effect, function, variables, compiler, id);
                let end = compiler.context.append_basic_block(function, &id.to_string());
                *id += 1;
                compiler.builder.build_unconditional_branch(end);
                (block, end)
            }
            None => {
                let end = compiler.context.append_basic_block(function, &id.to_string());
                *id += 1;
                (end, end)
            }
        };
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
        other = compile_block(effects.get(index + 1).unwrap().0.unwrap(), function, variables, compiler, id);
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