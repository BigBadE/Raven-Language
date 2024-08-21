use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::Arc;

use inkwell::basic_block::BasicBlock;
use inkwell::module::Linkage;
use inkwell::types::{BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;

use syntax::program::code::{ExpressionType, FinalizedEffectType, FinalizedEffects, FinalizedExpression};
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
        _ => {
            if is_modifier(types.inner_struct().data.modifiers, Modifier::Trait) {
                reference_struct(type_getter).as_basic_type_enum()
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
    type_getter: &mut CompilerTypeGetter<'ctx>,
) -> Option<BasicValueEnum<'ctx>> {
    // If the block already exists, go to it. If not, create it
    // Helps for weird control flow caused by certain blocks.
    let block = if let Some(block) = type_getter.blocks.get(&code.label) {
        type_getter.compiler.builder.position_at_end(block.clone());
        block.clone()
    } else {
        let temp = type_getter.compiler.context.append_basic_block(type_getter.function.unwrap(), &code.label);
        type_getter.blocks.insert(code.label.clone(), temp);
        temp
    };

    // Go to the end of the block to add code
    type_getter.current_block = Some(block);
    type_getter.compiler.builder.position_at_end(block);

    // Every block must break in some way, this checks that while compiling each line
    let mut broke = false;
    for line in &code.expressions {
        compile_line(line, type_getter, &mut broke);
    }

    // If this code block doesn't return, create an end block to jump to.
    if !code.returns {
        create_block_end(code, type_getter);
    }

    // Should never happen, but better than unsound code.
    if !broke {
        panic!("No break in code body!");
    }

    return None;
}

/// This is used by control flow altering blocks like for loops for convenience.
/// TODO rewrite this to be more sane. Probably smart.
pub fn create_block_end<'ctx>(code: &FinalizedCodeBody, type_getter: &mut CompilerTypeGetter<'ctx>) {
    let label = code.label.clone() + "end";
    let temp = if let Some(block) = type_getter.blocks.get(&label) {
        type_getter.compiler.builder.position_at_end(block.clone());
        block.clone()
    } else {
        type_getter.compiler.context.append_basic_block(type_getter.function.unwrap(), &label)
    };

    type_getter.blocks.insert(label, temp);
    type_getter.current_block = Some(temp);
    type_getter.compiler.builder.position_at_end(temp);
}

/// Compile a line of code, handling breaking
pub fn compile_line<'ctx>(line: &FinalizedExpression, type_getter: &mut CompilerTypeGetter<'ctx>, broke: &mut bool) {
    match line.expression_type {
        // If there's a return, return None for NOPs, else return the value
        ExpressionType::Return(_) => {
            if matches!(&line.effect.types, FinalizedEffectType::NOP) {
                type_getter.compiler.builder.build_return(None).unwrap();
            } else {
                let returned = compile_effect(type_getter, &line.effect).unwrap();
                type_getter.compiler.builder.build_return(Some(&returned)).unwrap();
            }
            *broke = true;
        }
        ExpressionType::Line => compile_nonreturning_line(line, type_getter, broke),
        // TODO implement breaks
        ExpressionType::Break => {
            compile_effect(type_getter, &line.effect);
            *broke = true;
        }
    }
}

/// Compiles a line that isn't a return or break.
fn compile_nonreturning_line<'ctx>(
    line: &FinalizedExpression,
    type_getter: &mut CompilerTypeGetter<'ctx>,
    broke: &mut bool,
) {
    if *broke {
        // If the function already broke, ignore anything other than code bodies.
        if matches!(&line.effect.types, FinalizedEffectType::CodeBody(_)) {
            compile_effect(type_getter, &line.effect);
        }
    } else {
        match &line.effect.types {
            FinalizedEffectType::CodeBody(body) => {
                // Make sure the code goes to the code body
                let destination = get_block_or_create(&body.label, type_getter);
                type_getter.compiler.builder.build_unconditional_branch(destination).unwrap();

                compile_effect(type_getter, &line.effect);
            }
            FinalizedEffectType::Jump(_) | FinalizedEffectType::CompareJump(_, _, _) => {
                *broke = true;
                compile_effect(type_getter, &line.effect);
            }
            _ => {
                compile_effect(type_getter, &line.effect);
            }
        }
    }
}

/// Compiles a single effect
// skipcq: RS-R1000 Match statements have complexity calculated incorrectly
pub fn compile_effect<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    effect: &FinalizedEffects,
) -> Option<BasicValueEnum<'ctx>> {
    if let Some(inner) = compile_simple_effects(type_getter, effect) {
        return inner;
    }

    return match &effect.types {
        FinalizedEffectType::CreateVariable(name, inner, types) => {
            let compiled = compile_effect(type_getter, inner).unwrap();
            type_getter.variables.insert(name.clone(), (types.clone(), compiled.as_basic_value_enum()));
            Some(compiled.as_basic_value_enum())
        }
        FinalizedEffectType::Jump(label) => {
            let destination = get_block_or_create(label, type_getter);
            type_getter.compiler.builder.build_unconditional_branch(destination).unwrap();
            None
        }
        FinalizedEffectType::CompareJump(effect, then_body, else_body) => {
            let effect = compile_effect(type_getter, effect).unwrap();
            let effect = load_if_pointer(type_getter, type_getter.compiler.context.i64_type(), effect).into_int_value();
            let then = get_block_or_create(then_body, type_getter);
            let else_block = get_block_or_create(else_body, type_getter);
            type_getter.compiler.builder.build_conditional_branch(effect, then, else_block).unwrap();
            None
        }
        //Sets pointer to value
        FinalizedEffectType::Set(setting, value) => {
            let output = compile_effect(type_getter, setting).unwrap();
            let value = compile_effect(type_getter, value).unwrap();
            let storing =
                load_if_pointer(type_getter, type_getter.compiler.context.ptr_type(AddressSpace::default()), value);
            type_getter.compiler.builder.build_store(output.into_pointer_value(), storing).unwrap();
            Some(output)
        }
        FinalizedEffectType::HeapStore(inner) => {
            let output = compile_effect(type_getter, inner).unwrap();

            let pointer_type = if output.get_type().is_pointer_type() {
                return Some(output);
            } else {
                output.get_type()
            };

            let malloc = malloc_type(type_getter, pointer_type.size_of().unwrap());
            let output =
                load_if_pointer(type_getter, type_getter.compiler.context.ptr_type(AddressSpace::default()), output);
            type_getter.compiler.builder.build_store(malloc, output).unwrap();
            Some(malloc.as_basic_value_enum())
        }
        FinalizedEffectType::StackStore(inner) => {
            let output = compile_effect(type_getter, inner).unwrap();
            if !output.is_pointer_value() {
                store_and_load(type_getter, output.get_type(), output)
            } else {
                Some(output)
            }
        }
        FinalizedEffectType::ReferenceLoad(inner) => {
            let inner = compile_effect(type_getter, inner).unwrap();
            let output = type_getter
                .compiler
                .builder
                .build_load(
                    type_getter.compiler.context.ptr_type(AddressSpace::default()),
                    inner.into_pointer_value(),
                    &type_getter.id.to_string(),
                )
                .unwrap();
            type_getter.id += 1;
            Some(output)
        }
        FinalizedEffectType::HeapAllocate(types) => {
            let output = type_getter.get_type(types);
            let malloc = malloc_type(type_getter, output.size_of().unwrap());

            Some(malloc.as_basic_value_enum())
        }
        _ => unreachable!(),
    };
}

/// Compiles a few effects handled entirely by seperate methods or simple one-liners
fn compile_simple_effects<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    effect: &FinalizedEffects,
) -> Option<Option<BasicValueEnum<'ctx>>> {
    return Some(match &effect.types {
        FinalizedEffectType::NOP => {
            panic!("Tried to compile a NOP! For {}", type_getter.function.unwrap().get_name().to_str().unwrap())
        }
        FinalizedEffectType::GenericMethodCall(func, types, _args) => {
            panic!("Tried to compile generic method call! {} and {}", func.data.name, types)
        }
        FinalizedEffectType::GenericVirtualCall(_, _, _, _) => {
            panic!("Generic virtual call not degeneric'd!")
        }
        FinalizedEffectType::CodeBody(body) => compile_block(body, type_getter),
        FinalizedEffectType::FunctionCall(_, calling_function, arguments, _) => {
            compile_function_call(type_getter, calling_function, arguments)
        }
        FinalizedEffectType::VirtualCall(func_offset, function, _, args) => {
            compile_virtual_call(type_getter, func_offset, function, args)
        }
        FinalizedEffectType::Downcast(base, target, functions) => compile_downcast(type_getter, base, target, functions),
        //Loads variable/field pointer from program, or self if program is None
        FinalizedEffectType::Load(loading_from, field, _) => compile_load(type_getter, loading_from, field),
        //Struct to create and a tuple of the index of the argument and the argument
        FinalizedEffectType::CreateStruct(effect, _, arguments) => compile_create_struct(type_getter, effect, arguments),
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
        FinalizedEffectType::LoadVariable(name) => Some(type_getter.variables.get(name).unwrap().1),
        _ => return None,
    });
}
fn compile_create_struct<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    effect: &Option<Box<FinalizedEffects>>,
    arguments: &Vec<(usize, FinalizedEffects)>,
) -> Option<BasicValueEnum<'ctx>> {
    let mut out_arguments = vec![MaybeUninit::uninit(); arguments.len()];

    for (index, effect) in arguments {
        let returned = compile_effect(type_getter, effect).unwrap();
        *out_arguments.get_mut(*index).unwrap() = MaybeUninit::new(returned);
    }

    let pointer = compile_effect(type_getter, effect.as_ref().unwrap()).unwrap().into_pointer_value();
    type_getter.id += 1;

    let fields = out_arguments.iter().map(|argument| unsafe { argument.assume_init() }.get_type()).collect::<Vec<_>>();
    let structure = type_getter.compiler.context.struct_type(fields.as_slice(), false);

    let mut offset = 0;
    for argument in out_arguments {
        let value = unsafe { argument.assume_init() };

        let pointer =
            type_getter.compiler.builder.build_struct_gep(structure, pointer, offset, &type_getter.id.to_string()).unwrap();
        type_getter.id += 1;
        type_getter.compiler.builder.build_store(pointer, value).unwrap();
        offset += 1;
    }

    return Some(pointer.as_basic_value_enum());
}

/// Compiles a load effect
fn compile_load<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    loading_from: &FinalizedEffects,
    field: &String,
) -> Option<BasicValueEnum<'ctx>> {
    let from = compile_effect(type_getter, loading_from).unwrap();
    let mut structure = loading_from.types.get_nongeneric_return(type_getter).unwrap();
    type_getter.fix_generic_struct(&mut structure);
    let structure = structure.inner_struct();
    let offset = structure.fields.iter().position(|struct_field| &struct_field.field.name == field).unwrap();

    let fields = structure.fields.iter().map(|field| type_getter.get_type(&field.field.field_type)).collect::<Vec<_>>();

    let gep = type_getter
        .compiler
        .builder
        .build_struct_gep(
            type_getter.compiler.context.struct_type(fields.as_slice(), false),
            from.into_pointer_value(),
            offset as u32,
            &type_getter.id.to_string(),
        )
        .unwrap();
    type_getter.id += 2;
    return Some(
        type_getter
            .compiler
            .builder
            .build_load(
                type_getter.compiler.context.ptr_type(AddressSpace::default()),
                gep,
                &(type_getter.id - 1).to_string(),
            )
            .unwrap(),
    );
}
fn compile_virtual_call<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    func_offset: &usize,
    function: &Arc<CodelessFinalizedFunction>,
    args: &Vec<FinalizedEffects>,
) -> Option<BasicValueEnum<'ctx>> {
    let table = compile_effect(type_getter, &args[0]).unwrap();

    let mut compiled_args = Vec::default();
    let calling = type_getter
        .compiler
        .builder
        .build_load(
            type_getter.compiler.context.ptr_type(AddressSpace::default()),
            table.into_pointer_value(),
            &type_getter.id.to_string(),
        )
        .unwrap();
    compiled_args.push(BasicMetadataValueEnum::from(calling));
    type_getter.id += 1;
    for i in 0..args.len() {
        compiled_args.push(BasicMetadataValueEnum::from(compile_effect(type_getter, &args[i]).unwrap()));
    }
    let reference_struct = reference_struct(type_getter);
    let table_pointer = type_getter
        .compiler
        .builder
        .build_struct_gep(reference_struct, table.into_pointer_value(), 1, &type_getter.id.to_string())
        .unwrap();
    type_getter.id += 1;
    let vtable = type_getter
        .compiler
        .builder
        .build_load(
            type_getter.compiler.context.ptr_type(AddressSpace::default()),
            table_pointer,
            &type_getter.id.to_string(),
        )
        .unwrap();
    type_getter.id += 1;
    let function_type = type_getter.get_function(function).get_type();
    let function_pointer = get_func_from_vtable(type_getter, vtable.into_pointer_value(), *func_offset);
    return type_getter
        .compiler
        .builder
        .build_indirect_call(
            function_type,
            function_pointer,
            compiled_args.into_boxed_slice().deref(),
            &(type_getter.id - 1).to_string(),
        )
        .unwrap()
        .try_as_basic_value()
        .left();
}

///Gets a function from a vtable
fn get_func_from_vtable<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    vtable: PointerValue<'ctx>,
    func_offset: usize,
) -> PointerValue<'ctx> {
    let mut struct_type = Vec::default();
    for _ in 0..=func_offset {
        struct_type.push(type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum());
    }
    let struct_type = type_getter.compiler.context.struct_type(struct_type.as_slice(), false);

    let function_pointer = type_getter
        .compiler
        .builder
        .build_struct_gep(struct_type, vtable, func_offset as u32, &type_getter.id.to_string())
        .unwrap();
    type_getter.id += 1;

    let function_pointer = type_getter
        .compiler
        .builder
        .build_load(
            type_getter.compiler.context.ptr_type(AddressSpace::default()),
            function_pointer,
            &type_getter.id.to_string(),
        )
        .unwrap()
        .into_pointer_value();
    type_getter.id += 1;
    return function_pointer;
}

/// Compiles a call to a function
fn compile_function_call<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    calling_function: &Arc<CodelessFinalizedFunction>,
    arguments: &Vec<FinalizedEffects>,
) -> Option<BasicValueEnum<'ctx>> {
    let mut final_arguments = Vec::default();

    let calling = type_getter.get_function(calling_function);
    type_getter.compiler.builder.position_at_end(type_getter.current_block.unwrap());

    for i in 0..arguments.len() {
        let argument = arguments.get(i).unwrap();
        let value = compile_effect(type_getter, argument).unwrap();

        final_arguments.push(From::from(value));
    }

    let call = type_getter
        .compiler
        .builder
        .build_call(calling, final_arguments.as_slice(), &type_getter.id.to_string())
        .unwrap()
        .try_as_basic_value()
        .left();
    type_getter.id += 1;
    return match call {
        Some(inner) => {
            if inner.is_pointer_value() {
                Some(inner)
            } else {
                let pointer = malloc_type(type_getter, inner.get_type().size_of().unwrap());
                type_getter.compiler.builder.build_store(pointer, inner).unwrap();
                Some(pointer.as_basic_value_enum())
            }
        }
        None => None,
    };
}

fn reference_struct<'ctx>(type_getter: &mut CompilerTypeGetter<'ctx>) -> StructType<'ctx> {
    return type_getter.compiler.context.struct_type(
        &[
            type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
            type_getter.compiler.context.ptr_type(AddressSpace::default()).as_basic_type_enum(),
        ],
        false,
    );
}

fn compile_downcast<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    base: &Box<FinalizedEffects>,
    target: &FinalizedTypes,
    functions: &Vec<Arc<CodelessFinalizedFunction>>,
) -> Option<BasicValueEnum<'ctx>> {
    let base_return_types = base.types.get_nongeneric_return(type_getter).unwrap();
    if is_modifier(base_return_types.inner_struct().data.modifiers, Modifier::Trait) {
        if !target.eq(&base_return_types) {
            panic!("Downcasting to a trait that doesn't match! Not implemented yet!")
        } else {
            return compile_effect(type_getter, base);
        }
    } else {
        let table = type_getter.vtable.clone();
        let base = compile_effect(type_getter, base).unwrap();
        let table = table.borrow_mut().get_vtable(type_getter, target, &base_return_types, functions);
        type_getter.id += 1;

        let structure = type_getter
            .compiler
            .context
            .struct_type(&[base.get_type(), table.as_pointer_value().get_type().as_basic_type_enum()], false);

        let malloc = malloc_type(type_getter, structure.size_of().unwrap());
        type_getter.compiler.builder.build_store(malloc, base).unwrap();

        let reference_struct = reference_struct(type_getter);
        let offset =
            type_getter.compiler.builder.build_struct_gep(reference_struct, malloc, 1, &type_getter.id.to_string()).unwrap();
        type_getter.id += 1;
        type_getter.compiler.builder.build_store(offset, table.as_basic_value_enum()).unwrap();
        return Some(malloc.as_basic_value_enum());
    }
}

fn load_if_pointer<'ctx, T: BasicType<'ctx>>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    types: T,
    storing: BasicValueEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    if storing.is_pointer_value() {
        let loaded = type_getter
            .compiler
            .builder
            .build_load(types, storing.into_pointer_value(), &type_getter.id.to_string())
            .unwrap();
        type_getter.id += 1;
        return loaded;
    }
    return storing;
}

/// Stores a value and then loads it
fn store_and_load<'ctx, T: BasicType<'ctx>>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    types: T,
    storing: BasicValueEnum<'ctx>,
) -> Option<BasicValueEnum<'ctx>> {
    let pointer = type_getter.compiler.builder.build_alloca(types, &type_getter.id.to_string()).unwrap();
    type_getter.id += 1;
    type_getter.compiler.builder.build_store(pointer, storing).unwrap();
    return Some(pointer.as_basic_value_enum());
}

/// Gets a block with the given name, and if it's not found, create it
fn get_block_or_create<'ctx>(name: &String, type_getter: &mut CompilerTypeGetter<'ctx>) -> BasicBlock<'ctx> {
    return if let Some(output) = type_getter.blocks.get(name) {
        output.clone()
    } else {
        let temp = type_getter.compiler.context.append_basic_block(type_getter.function.unwrap(), name);
        type_getter.compiler.builder.position_at_end(type_getter.current_block.unwrap());
        type_getter.blocks.insert(name.clone(), temp);
        temp
    };
}
