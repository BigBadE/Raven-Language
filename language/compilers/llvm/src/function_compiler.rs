use inkwell::basic_block::BasicBlock;
use inkwell::module::Linkage;
use inkwell::AddressSpace;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::Arc;

use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallableValue, FunctionValue,
};

use syntax::code::{ExpressionType, FinalizedEffects};
use syntax::function::{CodelessFinalizedFunction, FinalizedCodeBody};
use syntax::types::FinalizedTypes;
use syntax::{is_modifier, Attribute, Modifier};

use crate::internal::instructions::{compile_internal, malloc_type};
use crate::internal::intrinsics::compile_llvm_intrinsics;
use crate::type_getter::CompilerTypeGetter;
use crate::util::create_function_value;

pub fn instance_function<'a, 'ctx>(
    function: Arc<CodelessFinalizedFunction>,
    type_getter: &mut CompilerTypeGetter<'ctx>,
) -> FunctionValue<'ctx> {
    let value;
    if function.data.attributes.iter().any(|attribute| {
        if let Attribute::Basic(inner) = attribute {
            inner == "llvm_intrinsic"
        } else {
            false
        }
    }) {
        value =
            compile_llvm_intrinsics(function.data.name.split("::").last().unwrap(), type_getter);
    } else if is_modifier(function.data.modifiers, Modifier::Internal) {
        value = create_function_value(&function, type_getter, None);
        compile_internal(
            &type_getter,
            &type_getter.compiler,
            &function.data.name,
            value,
        );
    } else if is_modifier(function.data.modifiers, Modifier::Extern) {
        value = create_function_value(&function, type_getter, Some(Linkage::External))
    } else {
        value = create_function_value(&function, type_getter, None);
        unsafe { Arc::get_mut_unchecked(&mut type_getter.compiling) }.push((value, function));
    }
    return value;
}

pub fn instance_types<'ctx>(
    types: &FinalizedTypes,
    type_getter: &mut CompilerTypeGetter<'ctx>,
) -> BasicTypeEnum<'ctx> {
    return match types {
        FinalizedTypes::Reference(inner) => type_getter.get_type(inner),
        FinalizedTypes::Array(inner) => type_getter.get_type(inner),
        _ => {
            if is_modifier(types.inner_struct().data.modifiers, Modifier::Trait) {
                type_getter
                    .compiler
                    .context
                    .struct_type(
                        &[
                            type_getter
                                .compiler
                                .context
                                .i64_type()
                                .ptr_type(AddressSpace::default())
                                .as_basic_type_enum(),
                            type_getter
                                .compiler
                                .context
                                .i64_type()
                                .ptr_type(AddressSpace::default())
                                .as_basic_type_enum(),
                        ],
                        false,
                    )
                    .as_basic_type_enum()
            } else {
                let mut fields = vec![type_getter.compiler.context.i64_type().as_basic_type_enum()];
                for field in &types.inner_struct().fields {
                    fields.push(type_getter.get_type(&field.field.field_type));
                }

                type_getter
                    .compiler
                    .context
                    .struct_type(fields.as_slice(), true)
                    .as_basic_type_enum()
            }
        }
    };
}

pub fn compile_block<'ctx>(
    code: &FinalizedCodeBody,
    function: FunctionValue<'ctx>,
    type_getter: &mut CompilerTypeGetter<'ctx>,
    id: &mut u64,
) -> Option<BasicValueEnum<'ctx>> {
    let block = if let Some(block) = type_getter.blocks.get(&code.label) {
        type_getter.compiler.builder.position_at_end(block.clone());
        block.clone()
    } else {
        let temp = type_getter
            .compiler
            .context
            .append_basic_block(function, &code.label);
        type_getter.blocks.insert(code.label.clone(), temp);
        temp
    };

    type_getter.current_block = Some(block);
    type_getter.compiler.builder.position_at_end(block);
    let mut broke = false;
    for line in &code.expressions {
        match line.expression_type {
            ExpressionType::Return => {
                if let FinalizedEffects::CodeBody(body) = &line.effect {
                    if !broke {
                        let destination = unwrap_or_create(&body.label, function, type_getter);
                        type_getter
                            .compiler
                            .builder
                            .build_unconditional_branch(destination);
                    }
                    compile_effect(type_getter, function, &line.effect, id);
                    broke = true;
                }

                if matches!(&line.effect, FinalizedEffects::NOP) {
                    if !broke {
                        type_getter.compiler.builder.build_return(None);
                    }
                } else {
                    let returned = compile_effect(type_getter, function, &line.effect, id).unwrap();

                    if !broke {
                        type_getter.compiler.builder.build_return(Some(&returned));
                    }
                }
                broke = true;
            }
            ExpressionType::Line => {
                if broke {
                    if matches!(&line.effect, FinalizedEffects::CodeBody(_)) {
                        compile_effect(type_getter, function, &line.effect, id);
                    }
                } else {
                    match &line.effect {
                        FinalizedEffects::CodeBody(body) => {
                            let destination = unwrap_or_create(&body.label, function, type_getter);
                            type_getter
                                .compiler
                                .builder
                                .build_unconditional_branch(destination);

                            compile_effect(type_getter, function, &line.effect, id);

                            if !body.returns {
                                let label = body.label.clone() + "end";
                                let temp = if let Some(block) = type_getter.blocks.get(&label) {
                                    type_getter.compiler.builder.position_at_end(block.clone());
                                    block.clone()
                                } else {
                                    type_getter
                                        .compiler
                                        .context
                                        .append_basic_block(function, &label)
                                };

                                type_getter.blocks.insert(label, temp);
                                type_getter.current_block = Some(temp);
                                type_getter.compiler.builder.position_at_end(temp);
                            }
                        }
                        FinalizedEffects::Jump(_) | FinalizedEffects::CompareJump(_, _, _) => {
                            broke = true;
                            compile_effect(type_getter, function, &line.effect, id);
                        }
                        _ => {
                            compile_effect(type_getter, function, &line.effect, id);
                        }
                    }
                }
            }
            ExpressionType::Break => {
                return compile_effect(type_getter, function, &line.effect, id)
            }
        }
    }

    return None;
}

// skipcq: RS-R1000
pub fn compile_effect<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    function: FunctionValue<'ctx>,
    effect: &FinalizedEffects,
    id: &mut u64,
) -> Option<BasicValueEnum<'ctx>> {
    return match effect {
        FinalizedEffects::NOP => {
            panic!(
                "Tried to compile a NOP! For {}",
                function.get_name().to_str().unwrap()
            )
        }
        FinalizedEffects::CreateVariable(name, inner, types) => {
            let compiled = compile_effect(type_getter, function, inner, id).unwrap();
            type_getter.variables.insert(
                name.clone(),
                (types.clone(), compiled.as_basic_value_enum()),
            );
            Some(compiled.as_basic_value_enum())
        }
        //Label of jumping to body
        FinalizedEffects::Jump(label) => {
            let destination = unwrap_or_create(label, function, type_getter);
            type_getter
                .compiler
                .builder
                .build_unconditional_branch(destination);
            None
        }
        //Comparison effect, and label to jump to the first if true, second if false
        FinalizedEffects::CompareJump(effect, then_body, else_body) => {
            let effect = compile_effect(type_getter, function, effect, id).unwrap();
            let effect = if effect.is_pointer_value() {
                *id += 1;
                type_getter
                    .compiler
                    .builder
                    .build_load(effect.into_pointer_value(), &(*id - 1).to_string())
                    .into_int_value()
            } else {
                effect.into_int_value()
            };
            let then = unwrap_or_create(then_body, function, type_getter);
            let else_block = unwrap_or_create(else_body, function, type_getter);
            type_getter
                .compiler
                .builder
                .build_conditional_branch(effect, then, else_block);
            None
        }
        FinalizedEffects::CodeBody(body) => compile_block(body, function, type_getter, id),
        //Calling function, function arguments
        FinalizedEffects::MethodCall(pointer, calling_function, arguments) => {
            let mut final_arguments = Vec::default();

            let calling = type_getter.get_function(calling_function);
            type_getter
                .compiler
                .builder
                .position_at_end(type_getter.current_block.unwrap());

            if calling_function.return_type.is_some()
                && !calling.get_type().get_return_type().is_some()
            {
                let pointer = compile_effect(type_getter, function, pointer.as_ref().unwrap(), id)
                    .unwrap()
                    .into_pointer_value();
                final_arguments.push(From::from(pointer));

                add_args(
                    &mut final_arguments,
                    type_getter,
                    function,
                    arguments,
                    true,
                    id,
                );

                *id += 1;
                type_getter.compiler.builder.build_call(
                    calling,
                    final_arguments.as_slice(),
                    &(*id - 1).to_string(),
                );
                Some(pointer.as_basic_value_enum())
            } else {
                add_args(
                    &mut final_arguments,
                    type_getter,
                    function,
                    arguments,
                    false,
                    id,
                );

                let call = type_getter
                    .compiler
                    .builder
                    .build_call(calling, final_arguments.as_slice(), &id.to_string())
                    .try_as_basic_value()
                    .left();
                *id += 1;
                return match call {
                    Some(inner) => {
                        if inner.is_pointer_value() {
                            Some(inner)
                        } else {
                            let pointer = compile_effect(
                                type_getter,
                                function,
                                pointer.as_ref().unwrap(),
                                id,
                            )
                            .unwrap()
                            .into_pointer_value();
                            type_getter.compiler.builder.build_store(pointer, inner);
                            Some(pointer.as_basic_value_enum())
                        }
                    }
                    None => None,
                };
            }
        }
        //Sets pointer to value
        FinalizedEffects::Set(setting, value) => {
            let output = compile_effect(type_getter, function, setting, id).unwrap();
            let mut storing = compile_effect(type_getter, function, value, id).unwrap();
            if storing.is_pointer_value() {
                storing = type_getter
                    .compiler
                    .builder
                    .build_load(storing.into_pointer_value(), &id.to_string());
                *id += 1;
            }
            type_getter
                .compiler
                .builder
                .build_store(output.into_pointer_value(), storing);
            Some(output)
        }
        FinalizedEffects::LoadVariable(name) => {
            return Some(type_getter.variables.get(name).unwrap().1);
        }
        //Loads variable/field pointer from structure, or self if structure is None
        FinalizedEffects::Load(loading_from, field, _) => {
            let from = compile_effect(type_getter, function, loading_from, id).unwrap();
            //Compensate for type id
            let mut offset = 1;
            for struct_field in &loading_from
                .get_return(type_getter)
                .unwrap()
                .inner_struct()
                .fields
            {
                if &struct_field.field.name != field {
                    offset += 1;
                } else {
                    break;
                }
            }

            let gep = type_getter
                .compiler
                .builder
                .build_struct_gep(from.into_pointer_value(), offset, &id.to_string())
                .unwrap();
            *id += 2;
            Some(
                type_getter
                    .compiler
                    .builder
                    .build_load(gep, &(*id - 1).to_string()),
            )
        }
        //Struct to create and a tuple of the index of the argument and the argument
        FinalizedEffects::CreateStruct(effect, structure, arguments) => {
            let mut out_arguments = vec![MaybeUninit::uninit(); arguments.len()];

            for (index, effect) in arguments {
                let returned = compile_effect(type_getter, function, effect, id).unwrap();
                *out_arguments.get_mut(*index).unwrap() = MaybeUninit::new(returned);
            }

            let pointer = compile_effect(type_getter, function, effect.as_ref().unwrap(), id)
                .unwrap()
                .into_pointer_value();
            *id += 1;

            type_getter.compiler.builder.build_store(
                pointer,
                type_getter
                    .compiler
                    .context
                    .i64_type()
                    .const_int(structure.id(), false),
            );

            let mut offset = 1;
            for argument in out_arguments {
                let value = unsafe { argument.assume_init() };

                let pointer = type_getter
                    .compiler
                    .builder
                    .build_struct_gep(pointer, offset, &id.to_string())
                    .unwrap();
                *id += 1;
                type_getter.compiler.builder.build_store(pointer, value);
                offset += 1;
            }

            Some(pointer.as_basic_value_enum())
        }
        FinalizedEffects::Float(float) => Some(
            type_getter
                .compiler
                .context
                .f64_type()
                .const_float(*float)
                .as_basic_value_enum(),
        ),
        FinalizedEffects::UInt(int) => Some(
            type_getter
                .compiler
                .context
                .i64_type()
                .const_int(*int, false)
                .as_basic_value_enum(),
        ),
        FinalizedEffects::Bool(bool) => Some(
            type_getter
                .compiler
                .context
                .bool_type()
                .const_int(*bool as u64, false)
                .as_basic_value_enum(),
        ),
        FinalizedEffects::String(string) => Some(
            type_getter
                .compiler
                .context
                .const_string(string.as_bytes(), false)
                .as_basic_value_enum(),
        ),
        FinalizedEffects::Char(char) => Some(
            type_getter
                .compiler
                .context
                .i8_type()
                .const_int(*char as u64, false)
                .as_basic_value_enum(),
        ),
        FinalizedEffects::HeapStore(inner) => {
            let mut output = compile_effect(type_getter, function, inner, id).unwrap();

            let pointer_type = if output.get_type().is_pointer_type() {
                return Some(output);
            } else {
                output.get_type().ptr_type(AddressSpace::default())
            };

            let size = unsafe {
                type_getter.compiler.builder.build_gep(
                    pointer_type.const_zero(),
                    &[type_getter.compiler.context.i64_type().const_int(1, false)],
                    &id.to_string(),
                )
            };

            *id += 1;

            let malloc = type_getter
                .compiler
                .builder
                .build_call(
                    type_getter
                        .compiler
                        .module
                        .get_function("malloc")
                        .unwrap_or_else(|| compile_llvm_intrinsics("malloc", type_getter)),
                    &[BasicMetadataValueEnum::PointerValue(size)],
                    &id.to_string(),
                )
                .try_as_basic_value()
                .unwrap_left()
                .into_pointer_value();
            *id += 1;

            let malloc = type_getter.compiler.builder.build_pointer_cast(
                malloc,
                pointer_type,
                &id.to_string(),
            );
            *id += 1;

            if output.is_pointer_value() {
                output = type_getter
                    .compiler
                    .builder
                    .build_load(output.into_pointer_value(), &id.to_string());
                *id += 1;
            }
            type_getter.compiler.builder.build_store(malloc, output);
            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffects::StackStore(inner) => {
            let output = compile_effect(type_getter, function, inner, id).unwrap();
            if !output.is_pointer_value() {
                store_and_load(type_getter, output.get_type(), output, id)
            } else {
                Some(output)
            }
        }
        FinalizedEffects::ReferenceLoad(inner) => {
            let inner = compile_effect(type_getter, function, inner, id).unwrap();
            let output = type_getter
                .compiler
                .builder
                .build_load(inner.into_pointer_value(), &id.to_string());
            *id += 1;
            Some(output)
        }
        FinalizedEffects::HeapAllocate(types) => {
            let output = type_getter.get_type(types);

            let pointer_type = if output.is_pointer_type() {
                output.into_pointer_type()
            } else {
                output.ptr_type(AddressSpace::default())
            };

            let size = unsafe {
                type_getter.compiler.builder.build_gep(
                    pointer_type.const_zero(),
                    &[type_getter.compiler.context.i64_type().const_int(1, false)],
                    &id.to_string(),
                )
            };

            *id += 1;

            let size = type_getter
                .compiler
                .builder
                .build_bitcast(
                    size,
                    type_getter
                        .compiler
                        .context
                        .i64_type()
                        .ptr_type(AddressSpace::default()),
                    &id.to_string(),
                )
                .into_pointer_value();

            *id += 1;

            let malloc = type_getter
                .compiler
                .builder
                .build_call(
                    type_getter
                        .compiler
                        .module
                        .get_function("malloc")
                        .unwrap_or_else(|| compile_llvm_intrinsics("malloc", type_getter)),
                    &[BasicMetadataValueEnum::PointerValue(size)],
                    &id.to_string(),
                )
                .try_as_basic_value()
                .unwrap_left()
                .into_pointer_value();
            *id += 1;

            let malloc = type_getter.compiler.builder.build_pointer_cast(
                malloc,
                pointer_type,
                &id.to_string(),
            );
            *id += 1;

            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffects::CreateArray(types, values) => {
            let ptr_type = types
                .as_ref()
                .map(|inner| {
                    let inner = type_getter.get_type(inner);
                    unsafe {
                        type_getter.compiler.builder.build_gep(
                            if inner.is_pointer_type() {
                                inner.into_pointer_type()
                            } else {
                                inner.ptr_type(AddressSpace::default())
                            }
                            .const_zero(),
                            &[type_getter
                                .compiler
                                .context
                                .i64_type()
                                .const_int(values.len() as u64 + 1, false)],
                            &id.to_string(),
                        )
                    }
                })
                .unwrap_or_else(|| {
                    type_getter
                        .compiler
                        .context
                        .struct_type(&[], false)
                        .ptr_type(AddressSpace::default())
                        .const_zero()
                });
            let malloc = malloc_type(type_getter, ptr_type, id);

            type_getter.compiler.builder.build_store(
                malloc,
                type_getter
                    .compiler
                    .context
                    .i64_type()
                    .const_int(values.len() as u64, false),
            );

            let mut i = 1;
            for value in values {
                let gep = unsafe {
                    type_getter.compiler.builder.build_gep(
                        malloc,
                        &[type_getter.compiler.context.i64_type().const_int(i, false)],
                        &id.to_string(),
                    )
                };
                i += 1;
                *id += 1;
                let effect = compile_effect(type_getter, function, value, id).unwrap();
                type_getter.compiler.builder.build_store(gep, effect);
            }

            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffects::VirtualCall(func_offset, method, args) => {
            let table = compile_effect(type_getter, function, &args[0], id).unwrap();

            let mut compiled_args = Vec::default();
            let calling = compile_effect(type_getter, function, &args[0], id).unwrap();
            let calling = type_getter
                .compiler
                .builder
                .build_load(calling.into_pointer_value(), &id.to_string());
            compiled_args.push(BasicMetadataValueEnum::from(calling));
            *id += 1;
            for i in 1..args.len() {
                compiled_args.push(BasicMetadataValueEnum::from(
                    compile_effect(type_getter, function, &args[i], id).unwrap(),
                ));
            }
            let mut struct_type = Vec::default();
            for _ in 0..=*func_offset {
                struct_type.push(
                    type_getter
                        .get_function(method)
                        .get_type()
                        .ptr_type(AddressSpace::default())
                        .as_basic_type_enum(),
                );
            }

            let table = type_getter.compiler.builder.build_bitcast(
                table.into_pointer_value(),
                type_getter
                    .compiler
                    .context
                    .struct_type(
                        &[
                            type_getter
                                .compiler
                                .context
                                .i64_type()
                                .ptr_type(AddressSpace::default())
                                .as_basic_type_enum(),
                            type_getter
                                .compiler
                                .context
                                .struct_type(struct_type.as_slice(), false)
                                .ptr_type(AddressSpace::default())
                                .as_basic_type_enum(),
                        ],
                        false,
                    )
                    .ptr_type(AddressSpace::default()),
                &id.to_string(),
            );
            *id += 1;
            let offset = type_getter
                .compiler
                .builder
                .build_struct_gep(table.into_pointer_value(), 1, &id.to_string())
                .unwrap();
            *id += 1;
            let offset = type_getter
                .compiler
                .builder
                .build_load(offset, &id.to_string());
            *id += 1;
            let offset = type_getter
                .compiler
                .builder
                .build_struct_gep(
                    offset.into_pointer_value(),
                    *func_offset as u32,
                    &id.to_string(),
                )
                .unwrap();
            *id += 1;
            let offset = type_getter
                .compiler
                .builder
                .build_load(offset, &id.to_string())
                .into_pointer_value();
            *id += 2;
            type_getter
                .compiler
                .builder
                .build_call(
                    CallableValue::try_from(offset).unwrap(),
                    compiled_args.into_boxed_slice().deref(),
                    &(*id - 1).to_string(),
                )
                .try_as_basic_value()
                .left()
        }
        FinalizedEffects::Downcast(base, target) => {
            let found = base.get_return(type_getter).unwrap();
            if is_modifier(found.inner_struct().data.modifiers, Modifier::Trait) {
                if !target.eq(&found) {
                    panic!("Downcasting to a trait that doesn't match! Not implemented yet!")
                } else {
                    compile_effect(type_getter, function, base, id)
                }
            } else {
                let mut table = type_getter.vtable.clone();
                let base = compile_effect(type_getter, function, base, id).unwrap();
                let table = unsafe { Arc::get_mut_unchecked(&mut table) }.get_vtable(
                    type_getter,
                    &found,
                    &target,
                );
                *id += 1;

                let structure = type_getter.compiler.context.struct_type(
                    &[
                        base.get_type(),
                        table.as_pointer_value().get_type().as_basic_type_enum(),
                    ],
                    false,
                );

                let malloc = malloc_type(
                    type_getter,
                    structure.ptr_type(AddressSpace::default()).const_zero(),
                    id,
                );
                type_getter.compiler.builder.build_store(malloc, base);

                let offset = type_getter
                    .compiler
                    .builder
                    .build_struct_gep(malloc, 1, &id.to_string())
                    .unwrap();
                *id += 1;
                type_getter
                    .compiler
                    .builder
                    .build_store(offset, table.as_basic_value_enum());

                Some(malloc.as_basic_value_enum())
            }
        }
        FinalizedEffects::GenericMethodCall(func, types, _args) => {
            panic!(
                "Tried to compile generic method call! {} and {}",
                func.data.name, types
            )
        }
        FinalizedEffects::GenericVirtualCall(_, _, _, _) => {
            panic!("Generic virtual call not degeneric'd!")
        }
    };
}

fn store_and_load<'ctx, T: BasicType<'ctx>>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    types: T,
    inputer: BasicValueEnum<'ctx>,
    id: &mut u64,
) -> Option<BasicValueEnum<'ctx>> {
    let pointer = type_getter
        .compiler
        .builder
        .build_alloca(types, &id.to_string());
    *id += 1;
    type_getter.compiler.builder.build_store(pointer, inputer);
    return Some(pointer.as_basic_value_enum());
}

fn add_args<'ctx, 'a>(
    final_arguments: &'a mut Vec<BasicMetadataValueEnum<'ctx>>,
    type_getter: &mut CompilerTypeGetter<'ctx>,
    function: FunctionValue<'ctx>,
    arguments: &'a Vec<FinalizedEffects>,
    offset: bool,
    id: &mut u64,
) {
    for i in offset as usize..arguments.len() {
        let argument = arguments.get(i).unwrap();
        let value = compile_effect(type_getter, function, argument, id).unwrap();

        final_arguments.push(From::from(value));
    }
}

fn unwrap_or_create<'ctx>(
    name: &String,
    function: FunctionValue<'ctx>,
    type_getter: &mut CompilerTypeGetter<'ctx>,
) -> BasicBlock<'ctx> {
    return if let Some(output) = type_getter.blocks.get(name) {
        output.clone()
    } else {
        let temp = type_getter
            .compiler
            .context
            .append_basic_block(function, name);
        type_getter
            .compiler
            .builder
            .position_at_end(type_getter.current_block.unwrap());
        type_getter.blocks.insert(name.clone(), temp);
        temp
    };
}
