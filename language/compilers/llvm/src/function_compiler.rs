use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::Arc;

use inkwell::basic_block::BasicBlock;
use inkwell::module::Linkage;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue};
use inkwell::AddressSpace;

use syntax::program::code::{ExpressionType, FinalizedEffectType, FinalizedEffects};
use syntax::program::function::{CodelessFinalizedFunction, FinalizedCodeBody};
use syntax::program::types::FinalizedTypes;
use syntax::{is_modifier, Attribute, Modifier};

use crate::internal::instructions::{compile_internal, malloc_type};
use crate::internal::intrinsics::compile_llvm_intrinsics;
use crate::type_getter::CompilerTypeGetter;
use crate::util::create_function_value;

/// Instances a FunctionValue from its CodelessFinalizedFunction
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
        value = compile_llvm_intrinsics(function.data.name.split("::").last().unwrap(), type_getter);
    } else if is_modifier(function.data.modifiers, Modifier::Internal) {
        value = create_function_value(&function, type_getter, None);
        compile_internal(type_getter, &function, value);
    } else if is_modifier(function.data.modifiers, Modifier::Extern) {
        value = create_function_value(&function, type_getter, Some(Linkage::External))
    } else {
        value = create_function_value(&function, type_getter, None);
        type_getter.compiling.borrow_mut().push((value, function));
    }
    return value;
}

/// Instances a type from its FinalizedTypes
pub fn instance_types<'ctx>(types: &FinalizedTypes, type_getter: &mut CompilerTypeGetter<'ctx>) -> BasicTypeEnum<'ctx> {
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
                            type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
                            type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
                        ],
                        false,
                    )
                    .as_basic_type_enum()
            } else {
                let mut fields = vec![type_getter.compiler.context.i64_type().as_basic_type_enum()];
                for field in &types.inner_struct().fields {
                    fields.push(type_getter.get_type(&field.field.field_type));
                }

                type_getter.compiler.context.struct_type(fields.as_slice(), true).as_basic_type_enum()
            }
        }
    };
}

/// Compiles a FinalizedCodeBody
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
        let temp = type_getter.compiler.context.append_basic_block(function, &code.label);
        type_getter.blocks.insert(code.label.clone(), temp);
        temp
    };

    type_getter.current_block = Some(block);
    type_getter.compiler.builder.position_at_end(block);
    let mut broke = false;
    for line in &code.expressions {
        match line.expression_type {
            ExpressionType::Return(_) => {
                if let FinalizedEffectType::CodeBody(body) = &line.effect.types {
                    if !broke {
                        let destination = get_block_or_create(&body.label, function, type_getter);
                        type_getter.compiler.builder.build_unconditional_branch(destination).unwrap();
                    }
                    compile_effect(type_getter, function, &line.effect, id);
                    broke = true;
                }

                if matches!(&line.effect.types, FinalizedEffectType::NOP) {
                    if !broke {
                        type_getter.compiler.builder.build_return(None).unwrap();
                    }
                } else {
                    let returned = compile_effect(type_getter, function, &line.effect, id).unwrap();

                    if !broke {
                        type_getter.compiler.builder.build_return(Some(&returned)).unwrap();
                    }
                }
                broke = true;
            }
            ExpressionType::Line => {
                if broke {
                    if matches!(&line.effect.types, FinalizedEffectType::CodeBody(_)) {
                        compile_effect(type_getter, function, &line.effect, id);
                    }
                } else {
                    match &line.effect.types {
                        FinalizedEffectType::CodeBody(body) => {
                            let destination = get_block_or_create(&body.label, function, type_getter);
                            type_getter.compiler.builder.build_unconditional_branch(destination).unwrap();

                            compile_effect(type_getter, function, &line.effect, id);

                            if !body.returns {
                                let label = body.label.clone() + "end";
                                let temp = if let Some(block) = type_getter.blocks.get(&label) {
                                    type_getter.compiler.builder.position_at_end(block.clone());
                                    block.clone()
                                } else {
                                    type_getter.compiler.context.append_basic_block(function, &label)
                                };

                                type_getter.blocks.insert(label, temp);
                                type_getter.current_block = Some(temp);
                                type_getter.compiler.builder.position_at_end(temp);
                            }
                        }
                        FinalizedEffectType::Jump(_) | FinalizedEffectType::CompareJump(_, _, _) => {
                            broke = true;
                            compile_effect(type_getter, function, &line.effect, id);
                        }
                        _ => {
                            compile_effect(type_getter, function, &line.effect, id);
                        }
                    }
                }
            }
            ExpressionType::Break => return compile_effect(type_getter, function, &line.effect, id),
        }
    }

    return None;
}

/// Compiles a single effect
// skipcq: RS-R1000 Match statements have complexity calculated incorrectly
pub fn compile_effect<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    function: FunctionValue<'ctx>,
    effect: &FinalizedEffects,
    id: &mut u64,
) -> Option<BasicValueEnum<'ctx>> {
    return match &effect.types {
        FinalizedEffectType::NOP => {
            panic!("Tried to compile a NOP! For {}", function.get_name().to_str().unwrap())
        }
        FinalizedEffectType::CreateVariable(name, inner, types) => {
            let compiled = compile_effect(type_getter, function, inner, id).unwrap();
            type_getter.variables.insert(name.clone(), (types.clone(), compiled.as_basic_value_enum()));
            Some(compiled.as_basic_value_enum())
        }
        //Label of jumping to body
        FinalizedEffectType::Jump(label) => {
            let destination = get_block_or_create(label, function, type_getter);
            type_getter.compiler.builder.build_unconditional_branch(destination).unwrap();
            None
        }
        //Comparison effect, and label to jump to the first if true, second if false
        FinalizedEffectType::CompareJump(effect, then_body, else_body) => {
            let effect = compile_effect(type_getter, function, effect, id).unwrap();
            let effect = if effect.is_pointer_value() {
                *id += 1;
                type_getter
                    .compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), effect.into_pointer_value(), &(*id - 1).to_string())
                    .unwrap()
                    .into_int_value()
            } else {
                effect.into_int_value()
            };
            let then = get_block_or_create(then_body, function, type_getter);
            let else_block = get_block_or_create(else_body, function, type_getter);
            type_getter.compiler.builder.build_conditional_branch(effect, then, else_block).unwrap();
            None
        }
        FinalizedEffectType::CodeBody(body) => compile_block(body, function, type_getter, id),
        //Calling function, function arguments
        FinalizedEffectType::MethodCall(_calling_on, calling_function, arguments, _) => {
            let mut final_arguments = Vec::default();

            let calling = type_getter.get_function(calling_function);
            type_getter.compiler.builder.position_at_end(type_getter.current_block.unwrap());

            add_args(&mut final_arguments, type_getter, function, arguments, false, id);

            let call = type_getter
                .compiler
                .builder
                .build_call(calling, final_arguments.as_slice(), &id.to_string())
                .unwrap()
                .try_as_basic_value()
                .left();
            *id += 1;
            return match call {
                Some(inner) => {
                    if inner.is_pointer_value() {
                        Some(inner)
                    } else {
                        let pointer = malloc_type(type_getter, inner.get_type().size_of().unwrap(), id);
                        type_getter.compiler.builder.build_store(pointer, inner).unwrap();
                        Some(pointer.as_basic_value_enum())
                    }
                }
                None => None,
            };
        }
        //Sets pointer to value
        FinalizedEffectType::Set(setting, value) => {
            let output = compile_effect(type_getter, function, setting, id).unwrap();
            let mut storing = compile_effect(type_getter, function, value, id).unwrap();
            if storing.is_pointer_value() {
                storing = type_getter
                    .compiler
                    .builder
                    .build_load(
                        type_getter.compiler.context.ptr_type(AddressSpace::default()),
                        storing.into_pointer_value(),
                        &id.to_string(),
                    )
                    .unwrap();
                *id += 1;
            }
            let output = type_getter
                .compiler
                .builder
                .build_bit_cast(output, type_getter.compiler.context.ptr_type(AddressSpace::default()), &id.to_string())
                .unwrap();
            type_getter.compiler.builder.build_store(output.into_pointer_value(), storing).unwrap();
            Some(output)
        }
        FinalizedEffectType::LoadVariable(name) => {
            return Some(type_getter.variables.get(name).unwrap().1);
        }
        //Loads variable/field pointer from program, or self if program is None
        FinalizedEffectType::Load(loading_from, field, _) => {
            let from = compile_effect(type_getter, function, loading_from, id).unwrap();
            let mut offset = 0;
            let mut structure = loading_from.types.get_nongeneric_return(type_getter).unwrap();
            type_getter.fix_generic_struct(&mut structure);
            let structure = structure.inner_struct();
            for struct_field in &structure.fields {
                if &struct_field.field.name != field {
                    offset += 1;
                } else {
                    break;
                }
            }
            let fields =
                structure.fields.iter().map(|field| type_getter.get_type(&field.field.field_type)).collect::<Vec<_>>();

            let gep = type_getter
                .compiler
                .builder
                .build_struct_gep(
                    type_getter.compiler.context.struct_type(fields.as_slice(), false),
                    from.into_pointer_value(),
                    offset,
                    &id.to_string(),
                )
                .unwrap();
            *id += 2;
            Some(
                type_getter
                    .compiler
                    .builder
                    .build_load(type_getter.compiler.context.ptr_type(AddressSpace::default()), gep, &(*id - 1).to_string())
                    .unwrap(),
            )
        }
        //Struct to create and a tuple of the index of the argument and the argument
        FinalizedEffectType::CreateStruct(effect, _structure, arguments) => {
            let mut out_arguments = vec![MaybeUninit::uninit(); arguments.len()];

            for (index, effect) in arguments {
                let returned = compile_effect(type_getter, function, effect, id).unwrap();
                *out_arguments.get_mut(*index).unwrap() = MaybeUninit::new(returned);
            }

            let pointer = compile_effect(type_getter, function, effect.as_ref().unwrap(), id).unwrap().into_pointer_value();
            *id += 1;

            let fields =
                out_arguments.iter().map(|argument| unsafe { argument.assume_init() }.get_type()).collect::<Vec<_>>();
            let structure = type_getter.compiler.context.struct_type(fields.as_slice(), false);

            let mut offset = 0;
            for argument in out_arguments {
                let value = unsafe { argument.assume_init() };

                let pointer =
                    type_getter.compiler.builder.build_struct_gep(structure, pointer, offset, &id.to_string()).unwrap();
                *id += 1;
                type_getter.compiler.builder.build_store(pointer, value).unwrap();
                offset += 1;
            }

            Some(pointer.as_basic_value_enum())
        }
        FinalizedEffectType::Float(float) => {
            Some(type_getter.compiler.context.f64_type().const_float(*float).as_basic_value_enum())
        }
        FinalizedEffectType::UInt(int) => {
            Some(type_getter.compiler.context.i64_type().const_int(*int, false).as_basic_value_enum())
        }
        FinalizedEffectType::Bool(bool) => {
            Some(type_getter.compiler.context.bool_type().const_int(*bool as u64, false).as_basic_value_enum())
        }
        FinalizedEffectType::String(string) => {
            Some(type_getter.compiler.context.const_string(string.as_bytes(), false).as_basic_value_enum())
        }
        FinalizedEffectType::Char(char) => {
            Some(type_getter.compiler.context.i8_type().const_int(*char as u64, false).as_basic_value_enum())
        }
        FinalizedEffectType::HeapStore(inner) => {
            let mut output = compile_effect(type_getter, function, inner, id).unwrap();

            let pointer_type = if output.get_type().is_pointer_type() {
                return Some(output);
            } else {
                output.get_type()
            };

            let size = pointer_type.size_of().unwrap();

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
                    &[BasicMetadataValueEnum::IntValue(size)],
                    &id.to_string(),
                )
                .unwrap()
                .try_as_basic_value()
                .unwrap_left()
                .into_pointer_value();
            *id += 1;

            if output.is_pointer_value() {
                output = type_getter
                    .compiler
                    .builder
                    .build_load(
                        type_getter.compiler.context.ptr_type(AddressSpace::default()),
                        output.into_pointer_value(),
                        &id.to_string(),
                    )
                    .unwrap();
                *id += 1;
            }
            type_getter.compiler.builder.build_store(malloc, output).unwrap();
            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffectType::StackStore(inner) => {
            let output = compile_effect(type_getter, function, inner, id).unwrap();
            if !output.is_pointer_value() {
                store_and_load(type_getter, output.get_type(), output, id)
            } else {
                Some(output)
            }
        }
        FinalizedEffectType::ReferenceLoad(inner) => {
            let inner = compile_effect(type_getter, function, inner, id).unwrap();
            let output = type_getter
                .compiler
                .builder
                .build_load(
                    type_getter.compiler.context.ptr_type(AddressSpace::default()),
                    inner.into_pointer_value(),
                    &id.to_string(),
                )
                .unwrap();
            *id += 1;
            Some(output)
        }
        FinalizedEffectType::HeapAllocate(types) => {
            let output = type_getter.get_type(types);

            let pointer_type = if output.is_pointer_type() {
                output.into_pointer_type()
            } else {
                type_getter.compiler.context.ptr_type(AddressSpace::default())
            };

            let size = output.size_of().unwrap();

            *id += 1;

            let size = type_getter
                .compiler
                .builder
                .build_bit_cast(size, type_getter.compiler.context.ptr_type(AddressSpace::default()), &id.to_string())
                .unwrap()
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
                .unwrap()
                .try_as_basic_value()
                .unwrap_left()
                .into_pointer_value();
            *id += 1;

            let malloc = type_getter.compiler.builder.build_pointer_cast(malloc, pointer_type, &id.to_string()).unwrap();
            *id += 1;

            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffectType::CreateArray(types, values) => {
            let ptr_type = match types.as_ref() {
                Some(inner) => {
                    let inner = type_getter.get_type(inner);
                    let output = type_getter
                        .compiler
                        .builder
                        .build_int_mul(
                            inner.size_of().unwrap(),
                            type_getter.compiler.context.i64_type().const_int(values.len() as u64 + 1, false),
                            &id.to_string(),
                        )
                        .unwrap();
                    *id += 1;
                    output
                }
                None => type_getter.compiler.context.i64_type().const_zero(),
            };
            let malloc = malloc_type(type_getter, ptr_type, id);

            type_getter
                .compiler
                .builder
                .build_store(malloc, type_getter.compiler.context.i64_type().const_int(values.len() as u64, false))
                .unwrap();

            let malloc_int = type_getter
                .compiler
                .builder
                .build_ptr_to_int(malloc, type_getter.compiler.context.i64_type(), &id.to_string())
                .unwrap();
            *id += 1;

            let mut i = 1;
            for value in values {
                let field_pointer = type_getter
                    .compiler
                    .builder
                    .build_int_to_ptr(
                        type_getter
                            .compiler
                            .builder
                            .build_int_add(
                                malloc_int,
                                type_getter.compiler.context.i64_type().const_int(i, false),
                                &id.to_string(),
                            )
                            .unwrap(),
                        type_getter.compiler.context.ptr_type(AddressSpace::default()),
                        &(*id + 1).to_string(),
                    )
                    .unwrap();
                i += 1;
                *id += 2;
                let effect = compile_effect(type_getter, function, value, id).unwrap();
                *id += 1;
                type_getter.compiler.builder.build_store(field_pointer, effect).unwrap();
            }

            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffectType::VirtualCall(func_offset, method, _, args) => {
            let table = compile_effect(type_getter, function, &args[0], id).unwrap();

            let mut compiled_args = Vec::default();
            let calling = compile_effect(type_getter, function, &args[0], id).unwrap();
            let target_type = type_getter.compiler.context.ptr_type(AddressSpace::default());
            let calling = type_getter
                .compiler
                .builder
                .build_bit_cast(calling.into_pointer_value(), target_type, &id.to_string())
                .unwrap();
            *id += 1;
            let calling = type_getter
                .compiler
                .builder
                .build_load(
                    type_getter.compiler.context.ptr_type(AddressSpace::default()),
                    calling.into_pointer_value(),
                    &id.to_string(),
                )
                .unwrap();
            compiled_args.push(BasicMetadataValueEnum::from(calling));
            *id += 1;
            for i in 1..args.len() {
                compiled_args
                    .push(BasicMetadataValueEnum::from(compile_effect(type_getter, function, &args[i], id).unwrap()));
            }
            let mut struct_type = Vec::default();
            let function_type = type_getter.get_function(method).get_type();
            for _ in 0..=*func_offset {
                struct_type.push(type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum());
            }
            let struct_type = type_getter.compiler.context.struct_type(struct_type.as_slice(), false);
            let table_pointer = type_getter
                .compiler
                .builder
                .build_struct_gep(
                    type_getter.compiler.context.struct_type(
                        &[
                            type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
                            type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
                        ],
                        false,
                    ),
                    table.into_pointer_value(),
                    1,
                    &id.to_string(),
                )
                .unwrap();
            *id += 1;
            let vtable = type_getter
                .compiler
                .builder
                .build_load(type_getter.compiler.context.ptr_type(AddressSpace::default()), table_pointer, &id.to_string())
                .unwrap();
            *id += 1;
            let function_pointer = type_getter
                .compiler
                .builder
                .build_struct_gep(struct_type, vtable.into_pointer_value(), *func_offset as u32, &id.to_string())
                .unwrap();
            *id += 1;
            let offset = type_getter
                .compiler
                .builder
                .build_load(
                    type_getter.compiler.context.ptr_type(AddressSpace::default()),
                    function_pointer,
                    &id.to_string(),
                )
                .unwrap()
                .into_pointer_value();
            *id += 1;
            type_getter
                .compiler
                .builder
                .build_indirect_call(function_type, offset, compiled_args.into_boxed_slice().deref(), &(*id - 1).to_string())
                .unwrap()
                .try_as_basic_value()
                .left()
        }
        FinalizedEffectType::Downcast(base, target, functions) => {
            let base_return_types = base.types.get_nongeneric_return(type_getter).unwrap();
            if is_modifier(base_return_types.inner_struct().data.modifiers, Modifier::Trait) {
                if !target.eq(&base_return_types) {
                    panic!("Downcasting to a trait that doesn't match! Not implemented yet!")
                } else {
                    compile_effect(type_getter, function, base, id)
                }
            } else {
                let table = type_getter.vtable.clone();
                let base = compile_effect(type_getter, function, base, id).unwrap();
                let table = table.borrow_mut().get_vtable(type_getter, target, &base_return_types, functions);
                *id += 1;

                let structure = type_getter
                    .compiler
                    .context
                    .struct_type(&[base.get_type(), table.as_pointer_value().get_type().as_basic_type_enum()], false);
                let raw_structure = type_getter.compiler.context.struct_type(
                    &[
                        type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
                        type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
                    ],
                    false,
                );

                let malloc = malloc_type(type_getter, structure.size_of().unwrap(), id);
                let struct_field = type_getter
                    .compiler
                    .builder
                    .build_bit_cast(malloc, type_getter.compiler.context.ptr_type(AddressSpace::default()), &id.to_string())
                    .unwrap();
                *id += 1;
                type_getter.compiler.builder.build_store(struct_field.into_pointer_value(), base).unwrap();

                let offset =
                    type_getter.compiler.builder.build_struct_gep(raw_structure, malloc, 1, &id.to_string()).unwrap();
                *id += 2;
                type_getter.compiler.builder.build_store(offset, table.as_basic_value_enum()).unwrap();
                Some(
                    type_getter
                        .compiler
                        .builder
                        .build_bit_cast(
                            malloc,
                            type_getter.compiler.context.ptr_type(AddressSpace::default()),
                            &(*id - 1).to_string(),
                        )
                        .unwrap()
                        .as_basic_value_enum(),
                )
            }
        }
        FinalizedEffectType::GenericMethodCall(func, types, _args) => {
            panic!("Tried to compile generic method call! {} and {}", func.data.name, types)
        }
        FinalizedEffectType::GenericVirtualCall(_, _, _, _) => {
            panic!("Generic virtual call not degeneric'd!")
        }
    };
}

/// Stores a value and then loads it
fn store_and_load<'ctx, T: BasicType<'ctx>>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    types: T,
    inputer: BasicValueEnum<'ctx>,
    id: &mut u64,
) -> Option<BasicValueEnum<'ctx>> {
    let pointer = type_getter.compiler.builder.build_alloca(types, &id.to_string()).unwrap();
    *id += 1;
    type_getter.compiler.builder.build_store(pointer, inputer).unwrap();
    return Some(pointer.as_basic_value_enum());
}

/// Adds the arguments to final_arguments
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

/// Gets a block with the given name, and if it's not found, create it
fn get_block_or_create<'ctx>(
    name: &String,
    function: FunctionValue<'ctx>,
    type_getter: &mut CompilerTypeGetter<'ctx>,
) -> BasicBlock<'ctx> {
    return if let Some(output) = type_getter.blocks.get(name) {
        output.clone()
    } else {
        let temp = type_getter.compiler.context.append_basic_block(function, name);
        type_getter.compiler.builder.position_at_end(type_getter.current_block.unwrap());
        type_getter.blocks.insert(name.clone(), temp);
        temp
    };
}
