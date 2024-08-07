use std::sync::Arc;

use crate::compiler::CompilerImpl;
use crate::internal::intrinsics::compile_llvm_intrinsics;
use crate::internal::math_internal::math_internal;
use crate::internal::string_internal::string_internal;
use crate::type_getter::CompilerTypeGetter;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::AddressSpace;
use syntax::program::function::CodelessFinalizedFunction;

/// Compiles a method with the internal keyword
pub fn compile_internal<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    function: &Arc<CodelessFinalizedFunction>,
    value: FunctionValue<'ctx>,
) {
    let compiler = type_getter.compiler.clone();
    let name = &function.data.name;

    let block = compiler.context.append_basic_block(value, "0");
    compiler.builder.position_at_end(block);
    let params = value.get_params();
    if string_internal(type_getter, &compiler, name, &value) || math_internal(type_getter, &compiler, name, &value) {
        return;
    }
    if name.starts_with("types::pointer::Pointer<T>::get_ptr_data") {
        let pointer_int = compiler
            .builder
            .build_ptr_to_int(params.get(0).unwrap().into_pointer_value(), compiler.context.i64_type(), "0")
            .unwrap()
            .as_basic_value_enum();
        compiler.builder.build_return(Some(&pointer_int)).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::write_ptr_data$") {
        let pointer = compiler
            .builder
            .build_load(compiler.context.i64_type(), params.get(0).unwrap().into_pointer_value(), "0")
            .unwrap();
        let pointer = compiler
            .builder
            .build_int_to_ptr(pointer.into_int_value(), compiler.context.ptr_type(AddressSpace::default()), "1")
            .unwrap();
        let data = compiler
            .builder
            .build_load(compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "2")
            .unwrap();
        compiler.builder.build_store(pointer, data).unwrap();
        compiler.builder.build_return(None).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::get_size$") {
        let storing = malloc_type(type_getter, type_getter.compiler.context.i64_type().size_of(), &mut 0);
        let target_type = type_getter.get_type(function.generics.iter().next().unwrap().1);
        compiler.builder.build_store(storing, target_type.size_of().unwrap().as_basic_value_enum()).unwrap();
        compiler.builder.build_return(Some(&storing)).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::read") {
        let storing = malloc_type(type_getter, type_getter.compiler.context.i64_type().size_of(), &mut 0);
        let pointer_val = compiler
            .builder
            .build_load(compiler.context.i64_type(), params[0].into_pointer_value(), "1")
            .unwrap()
            .into_int_value();
        type_getter
            .compiler
            .builder
            .build_store(
                storing,
                compiler
                    .builder
                    .build_int_to_ptr(pointer_val, compiler.context.ptr_type(AddressSpace::default()), "2")
                    .unwrap(),
            )
            .unwrap();
        compiler.builder.build_return(Some(&storing)).unwrap();
    } else if name.starts_with("numbers::Cast") {
        build_cast(value.get_params().first().unwrap(), value.get_type().get_return_type().unwrap(), &compiler);
    } else if name.starts_with("math::RightShift") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, type_getter.compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_right_shift(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), pointer_type, "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                true,
                "1",
            )
            .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
    } else if name.starts_with("math::LogicRightShift") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, type_getter.compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_right_shift(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), pointer_type, "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                false,
                "1",
            )
            .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
    } else if name.starts_with("math::LeftShift") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, type_getter.compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_left_shift(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), pointer_type, "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                "1",
            )
            .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
    } else if name.starts_with("array::Index") {
        let offset = get_loaded(&compiler, compiler.context.i64_type(), params.get(1).unwrap()).into_int_value();
        let offset = compiler.builder.build_int_add(offset, compiler.context.i64_type().const_int(1, false), "3").unwrap();

        let gep;
        unsafe {
            gep = compiler
                .builder
                .build_in_bounds_gep(
                    type_getter.compiler.context.ptr_type(AddressSpace::default()),
                    params.first().unwrap().into_pointer_value(),
                    &[offset],
                    "1",
                )
                .unwrap();
        }

        let gep =
            compiler.builder.build_load(type_getter.compiler.context.ptr_type(AddressSpace::default()), gep, "2").unwrap();
        compiler.builder.build_return(Some(&gep)).unwrap();
    } else if name.starts_with("array::Empty") {
        let size = unsafe {
            type_getter
                .compiler
                .builder
                .build_gep(
                    type_getter.compiler.context.ptr_type(AddressSpace::default()),
                    type_getter.compiler.context.ptr_type(AddressSpace::default()).const_zero(),
                    &[type_getter.compiler.context.i64_type().const_int(1, false)],
                    "0",
                )
                .unwrap()
        };

        let malloc = compiler
            .builder
            .build_call(
                compiler.module.get_function("malloc").unwrap_or_else(|| compile_llvm_intrinsics("malloc", type_getter)),
                &[BasicMetadataValueEnum::PointerValue(size)],
                "1",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_left()
            .into_pointer_value();

        compiler.builder.build_store(malloc, compiler.context.i64_type().const_zero()).unwrap();
        compiler.builder.build_return(Some(&malloc.as_basic_value_enum())).unwrap();
    } else {
        panic!("Unknown internal operation: {}", name)
    }
}

/// Creates a malloc for the type
pub fn malloc_type<'a>(type_getter: &CompilerTypeGetter<'a>, size: IntValue<'a>, id: &mut u64) -> PointerValue<'a> {
    let size = type_getter
        .compiler
        .builder
        .build_int_to_ptr(size, type_getter.compiler.context.ptr_type(AddressSpace::default()), &id.to_string())
        .unwrap();
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
    return malloc;
}

/// Loads the type if it's a pointer
fn get_loaded<'ctx, T: BasicType<'ctx>>(
    compiler: &CompilerImpl<'ctx>,
    pointer_type: T,
    value: &BasicValueEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    if value.is_pointer_value() {
        return compiler.builder.build_load(pointer_type, value.into_pointer_value(), "0").unwrap();
    }
    return *value;
}

/// Casts a number from one type to another
fn build_cast(first: &BasicValueEnum, _second: BasicTypeEnum, compiler: &CompilerImpl) {
    //TODO float casting
    compiler
        .builder
        .build_return(Some(
            &compiler
                .builder
                .build_load(compiler.context.ptr_type(AddressSpace::default()), first.into_pointer_value(), "1")
                .unwrap(),
        ))
        .unwrap();
}
