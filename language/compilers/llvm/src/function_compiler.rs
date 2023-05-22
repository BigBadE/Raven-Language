use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::Arc;
use inkwell::basic_block::BasicBlock;

use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use inkwell::types::{BasicType, StructType};

use syntax::{is_modifier, Modifier};
use syntax::code::{Effects, ExpressionType};
use syntax::function::{CodeBody, Function};
use syntax::r#struct::Struct;

use crate::internal::instructions::compile_internal;
use crate::type_getter::CompilerTypeGetter;
use crate::util::{create_function_value, print_formatted};

pub fn instance_function<'a, 'ctx>(function: Arc<Function>, type_getter: &mut CompilerTypeGetter<'ctx>) -> FunctionValue<'ctx> {
    let value = create_function_value(&function, type_getter);

    if is_modifier(function.modifiers, Modifier::Internal) {
        compile_internal(&type_getter.compiler, &function.name, &function.fields, value);
    } else if is_modifier(function.modifiers, Modifier::Extern) {
        todo!()
    } else {
        unsafe { Rc::get_mut_unchecked(&mut type_getter.compiling) }.push((value, function));
    }
    return value;
}

pub fn instance_struct<'ctx>(structure: Arc<Struct>, type_getter: &mut CompilerTypeGetter<'ctx>) -> StructType<'ctx> {
    let mut fields = vec!(type_getter.compiler.context.i64_type().as_basic_type_enum());
    for field in &structure.fields {
        fields.push(type_getter.get_type(&field.field.field_type));
    }

    return type_getter.compiler.context.struct_type(fields.as_slice(), true);
}

pub fn compile_block<'ctx>(code: &CodeBody, function: FunctionValue<'ctx>, type_getter: &mut CompilerTypeGetter<'ctx>,
                           id: &mut u64) -> Option<BasicValueEnum<'ctx>> {
    let block = if let Some(block) = type_getter.blocks.get(&code.label) {
        type_getter.compiler.builder.position_at_end(block.clone());
        block.clone()
    } else {
        println!("Creating {}", code.label);
        let temp = type_getter.compiler.context.append_basic_block(function, &code.label);
        type_getter.blocks.insert(code.label.clone(), temp);
        temp
    };

    type_getter.current_block = Some(block);
    type_getter.compiler.builder.position_at_end(block);
    for line in &code.expressions {
        match line.expression_type {
            ExpressionType::Return => {
                match &line.effect {
                    Effects::NOP() => {}
                    _ => {
                        let returned = compile_effect(type_getter, function, &line.effect, id).unwrap();

                        print_formatted(function.to_string());
                        if returned.is_struct_value() {
                            type_getter.compiler.builder.build_store(function.get_first_param().unwrap().into_pointer_value(),
                                                                     returned);
                            type_getter.compiler.builder.build_return(None);
                        } else if returned.is_pointer_value() &&
                            returned.into_pointer_value().get_type().get_element_type().is_struct_type() {
                            type_getter.compiler.builder.build_store(
                                function.get_first_param().unwrap().into_pointer_value(),
                                type_getter.compiler.builder.build_load(returned.into_pointer_value(), &id.to_string()));
                            *id += 1;
                            type_getter.compiler.builder.build_return(None);
                        } else {
                            type_getter.compiler.builder.build_return(Some(&returned));
                        }
                        break
                    }
                }
            }
            ExpressionType::Line => {
                compile_effect(type_getter, function, &line.effect, id);
            }
            ExpressionType::Break => return compile_effect(type_getter, function, &line.effect, id)
        }
    }

    return None;
}

pub fn compile_effect<'ctx>(type_getter: &mut CompilerTypeGetter<'ctx>, function: FunctionValue<'ctx>,
                            effect: &Effects, id: &mut u64) -> Option<BasicValueEnum<'ctx>> {
    return match effect {
        Effects::NOP() => panic!("Tried to compile a NOP"),
        Effects::Operation(_, _) => panic!("Checker failed to resolve operation!"),
        Effects::CreateVariable(name, inner) => {
            let compiled = compile_effect(type_getter, function, inner, id).unwrap();
            type_getter.variables.insert(name.clone(),
                                         (inner.get_return(type_getter).unwrap(),
                                          compiled.as_basic_value_enum()));
            Some(compiled.as_basic_value_enum())
        },
        //Label of jumping to body
        Effects::Jump(label) => {
            let destination = unwrap_or_create(label, function, type_getter);
            type_getter.compiler.builder.build_unconditional_branch(destination);
            None
        }
        //Comparison effect, and label to jump to the first if true, second if false
        Effects::CompareJump(effect, then_body, else_body) => {
            let effect = compile_effect(type_getter, function, effect, id).unwrap().into_int_value();
            let then = unwrap_or_create(then_body, function, type_getter);
            let else_block = unwrap_or_create(else_body, function, type_getter);
            type_getter.compiler.builder.build_conditional_branch(effect, then, else_block);
            None
        }
        Effects::CodeBody(body) => {
            let destination = unwrap_or_create(&body.label, function, type_getter);
            type_getter.compiler.builder.build_unconditional_branch(destination);
            compile_block(body, function, type_getter, id)
        }
        //Calling function, function arguments
        Effects::MethodCall(calling_function, arguments) => {
            let mut final_arguments = Vec::new();

            let calling = type_getter.get_function(calling_function);
            type_getter.compiler.builder.position_at_end(type_getter.current_block.unwrap());

            if calling_function.return_type.is_some() && !calling.get_type().get_return_type().is_some() {
                let types = type_getter.get_type(&calling_function.return_type.as_ref().unwrap());
                let pointer = type_getter.compiler.builder.build_alloca(
                    types, &id.to_string());
                *id += 1;
                final_arguments.push(From::from(pointer.as_basic_value_enum()));

                for argument in arguments {
                    final_arguments.push(From::from(compile_effect(type_getter, function, argument, id).unwrap()));
                }

                *id += 1;
                type_getter.compiler.builder.build_call(calling, final_arguments.as_slice(), &(*id - 1).to_string());
                Some(pointer.as_basic_value_enum())
            } else {
                for argument in arguments {
                    final_arguments.push(From::from(compile_effect(type_getter, function, argument, id).unwrap()));
                }

                *id += 1;
                type_getter.compiler.builder.build_call(calling, final_arguments.as_slice(),
                                                             &(*id - 1).to_string()).try_as_basic_value().left()
                    .map_or(None, |value| Some(value))
            }
        }
        //Sets pointer to value
        Effects::Set(setting, value) => {
            let output = compile_effect(type_getter, function, setting, id).unwrap();
            let storing = compile_effect(type_getter, function, value, id).unwrap();
            type_getter.compiler.builder.build_store(output.into_pointer_value(), storing);
            Some(output)
        }
        Effects::LoadVariable(name) => return Some(type_getter.variables.get(name).unwrap().1),
        //Loads variable/field pointer from structure, or self if structure is None
        Effects::Load(loading_from, field) => {
            let from = compile_effect(type_getter, function, loading_from, id).unwrap();
            //Compensate for type id
            let mut offset = 1;
            for struct_field in &loading_from
                .get_return(type_getter)
                .unwrap().clone_struct().fields {
                if &struct_field.field.name != field {
                    offset += 1;
                } else {
                    break;
                }
            }

            let pointer;
            if !from.is_pointer_value() {
                pointer = type_getter.compiler.builder.build_alloca(from.get_type(), &id.to_string());
                *id += 1;
                type_getter.compiler.builder.build_store(pointer, from.into_struct_value().as_basic_value_enum());
            } else {
                pointer = from.into_pointer_value();
            }
            *id += 2;
            Some(type_getter.compiler.builder.build_load(
                type_getter.compiler.builder.build_struct_gep(pointer, offset, &(*id - 2).to_string()).unwrap(),
                &(*id - 1).to_string()).as_basic_value_enum())
        }
        //Struct to create and a tuple of the index of the argument and the argument
        Effects::CreateStruct(structure, arguments) => {
            let mut out_arguments = vec![MaybeUninit::uninit(); arguments.len()];

            for (index, effect) in arguments {
                let returned = compile_effect(type_getter, function, effect, id).unwrap();
                *out_arguments.get_mut(*index).unwrap() = MaybeUninit::new(returned);
            }

            let storing = type_getter.get_type(structure);
            let pointer = type_getter.compiler.builder.build_alloca(storing, &id.to_string());
            *id += 1;

            type_getter.compiler.builder.build_store(pointer,
                                                     type_getter.compiler.context.i64_type()
                .const_int(structure.id(), false));

            let mut offset = 1;
            for argument in out_arguments {
                let value = unsafe { argument.assume_init() };

                let pointer = type_getter.compiler.builder.build_struct_gep(pointer, offset, &id.to_string()).unwrap();
                *id += 1;
                type_getter.compiler.builder.build_store(pointer, value);
                offset += 1;
            }

            Some(pointer.as_basic_value_enum())
        }
        Effects::Float(float) => Some(type_getter.compiler.context.f64_type().const_float(*float).as_basic_value_enum()),
        Effects::Int(int) => Some(type_getter.compiler.context.i64_type().const_int(*int as u64, false).as_basic_value_enum()),
        Effects::UInt(uint) => Some(type_getter.compiler.context.i64_type().const_int(*uint, true).as_basic_value_enum()),
        Effects::Bool(bool) => Some(type_getter.compiler.context.bool_type().const_int(*bool as u64, false).as_basic_value_enum()),
        Effects::String(string) => Some(type_getter.compiler.context.const_string(string.as_bytes(), false).as_basic_value_enum())
    };
}

fn unwrap_or_create<'ctx>(name: &String, function: FunctionValue<'ctx>, type_getter: &mut CompilerTypeGetter<'ctx>) -> BasicBlock<'ctx> {
    return if let Some(output) = type_getter.blocks.get(name) {
        output.clone()
    } else {
        println!("2 Creating {}", name);
        let temp = type_getter.compiler.context.append_basic_block(function, name);
        type_getter.compiler.builder.position_at_end(type_getter.current_block.unwrap());
        type_getter.blocks.insert(name.clone(), temp);
        temp
    }
}