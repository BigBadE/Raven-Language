use std::sync::Arc;

use crate::compiler::CompilerImpl;
use crate::internal::math_internal::math_internal;
use crate::type_getter::CompilerTypeGetter;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
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
    if math_internal(&compiler, name, &value) {
        return;
    }
    if name.starts_with("types::pointer::Pointer<T>::get_ptr_data") {
        let pointer_int = compiler
            .builder
            .build_ptr_to_int(params.get(0).unwrap().into_pointer_value(), compiler.context.i64_type(), "0")
            .unwrap()
            .as_basic_value_enum();
        compiler.builder.build_return(Some(&pointer_int)).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::malloc_size") {
        let size = compiler
            .builder
            .build_load(compiler.context.i64_type(), params.get(0).unwrap().into_pointer_value(), "0")
            .unwrap();
        compiler.builder.build_return(Some(&size)).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::write_ptr_data$") {
        let data = compiler
            .builder
            .build_load(compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "2")
            .unwrap();
        compiler.builder.build_store(params.get(0).unwrap().into_pointer_value(), data).unwrap();
        compiler.builder.build_return(None).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::get_size$") {
        let target_type = type_getter.get_type(function.generics.iter().next().unwrap().1);
        compiler.builder.build_return(Some(&target_type.size_of().unwrap().as_basic_value_enum())).unwrap();
    } else if name.starts_with("types::pointer::Pointer<T>::read_ptr_data$") {
        compiler.builder.build_return(Some(&params[0].into_pointer_value())).unwrap();
    } else if name.starts_with("numbers::Cast") {
        build_cast(value.get_params().first().unwrap(), value.get_type().get_return_type().unwrap(), &compiler);
    } else {
        panic!("Unknown internal operation: {}", name)
    }
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
