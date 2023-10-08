use inkwell::builder::Builder;
use inkwell::{AddressSpace, IntPredicate};
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallableValue, FunctionValue, PointerValue};
use crate::compiler::CompilerImpl;
use crate::internal::intrinsics::compile_llvm_intrinsics;
use crate::type_getter::CompilerTypeGetter;

pub fn compile_internal<'ctx>(type_getter: &CompilerTypeGetter<'ctx>, compiler: &CompilerImpl<'ctx>, name: &String, value: FunctionValue<'ctx>) {
    let block = compiler.context.append_basic_block(value, "0");
    compiler.builder.position_at_end(block);
    let params = value.get_params();
    if name.starts_with("numbers::cast_") {
        build_cast(value.get_params().get(0).unwrap(), value.get_type().get_return_type().unwrap(), compiler);
        return;
    } else if name.starts_with("math::add_u") || name.starts_with("math::add_i") {
        let pointer_type = params.get(0).unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type);

        let returning = compiler.builder.build_int_add(compiler.builder.build_load(pointer_type, "2").into_int_value(),
                                                       compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::subtract_u") || name.starts_with("math::subtract_i") {
        let pointer_type = params.get(0).unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type);
        let returning = compiler.builder.build_int_sub(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                       compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::multiply_u") || name.starts_with("math::multiply_i") {
        let pointer_type = params.get(0).unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type);
        let returning = compiler.builder.build_int_mul(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                       compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::divide_u") || name.starts_with("math::divide_i") {
        let pointer_type = params.get(0).unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type);
        let returning = if name.starts_with("math::divide_u") {
            compiler.builder.build_int_unsigned_div(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                  compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1")
        } else {
            compiler.builder.build_int_signed_div(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                                  compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1")
        };
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::remainder_u") || name.starts_with("math::remainder_i") {
        let pointer_type = params.get(0).unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type);
        let returning = if name.starts_with("math::remainder_u") {
            compiler.builder.build_int_unsigned_rem(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                    compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1")
        } else {
            compiler.builder.build_int_signed_rem(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                  compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1")
        };
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::equal_") {
        let returning = compiler.builder
            .build_int_compare(IntPredicate::EQ, compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                               compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
        compiler.builder.build_return(Some(&returning));
    } else if name.starts_with("array::index_") {
        let offset = get_loaded(&compiler.builder, params.get(1).unwrap()).into_int_value();
        let offset = compiler.builder.build_int_add(offset, compiler.context.i64_type().const_int(1, false), "3");

        let gep;
        unsafe {
            gep = compiler.builder
                .build_in_bounds_gep(params.get(0).unwrap().into_pointer_value(),
                                     &[offset], "1");
        }
        compiler.builder.build_return(Some(&compiler.builder.build_bitcast(gep, compiler.context.i64_type().ptr_type(AddressSpace::default()), "2")));
    } else if name.starts_with("array::empty") {
        let size = unsafe {
            type_getter.compiler.builder.build_gep(value.get_type().get_return_type().unwrap()
                                                       .ptr_type(AddressSpace::default()).const_zero(),
                                                   &[type_getter.compiler.context.i64_type()
                                                       .const_int(1, false)], "0")
        };

        let malloc = compiler.builder.build_call(compiler.module.get_function("malloc")
                                                     .unwrap_or(compile_llvm_intrinsics("malloc", type_getter)),
                                                 &[BasicMetadataValueEnum::PointerValue(size)], "1")
            .try_as_basic_value().unwrap_left().into_pointer_value();

        compiler.builder.build_store(malloc, compiler.context.i64_type().const_zero());
        compiler.builder.build_return(Some(&malloc.as_basic_value_enum()));
    } else if name.starts_with("main::test_internal") {
        let calling = compiler.module.get_global("str_vtable").unwrap().as_pointer_value();
        let calling = compiler.builder.build_bitcast(calling,
                                                     compile_llvm_intrinsics("printf", type_getter).get_type().ptr_type(AddressSpace::default()).ptr_type(AddressSpace::default()), "2");
        println!("Found {}", calling.get_type());
        let calling = compiler.builder.build_load(calling.into_pointer_value(), "4");
        compiler.builder.build_call(CallableValue::try_from(calling.into_pointer_value()).unwrap(),
        &[From::from(compiler.context.const_string("Testing".as_bytes(), true))], "1");
        compiler.builder.build_return(None);
    } else {
        panic!("Unknown internal operation: {}", name)
    }
}

fn malloc_type<'a>(type_getter: &CompilerTypeGetter<'a>, pointer_type: PointerValue<'a>) -> PointerValue<'a> {
    let size = unsafe {type_getter.compiler.builder.build_gep(pointer_type.get_type().const_zero(),
                                                              &[type_getter.compiler.context.i64_type().const_int(1, false)], "2")
    };

    let malloc = type_getter.compiler.builder.build_call(type_getter.compiler.module.get_function("malloc")
                                                             .unwrap_or(compile_llvm_intrinsics("malloc", type_getter)),
                                                         &[BasicMetadataValueEnum::PointerValue(size)], "0").try_as_basic_value().unwrap_left().into_pointer_value();
    let malloc = type_getter.compiler.builder.build_bitcast(malloc.as_basic_value_enum(), pointer_type.as_basic_value_enum().get_type(), "1");
    return malloc.into_pointer_value();
}

fn get_loaded<'ctx>(compiler: &Builder<'ctx>, value: &BasicValueEnum<'ctx>) -> BasicValueEnum<'ctx> {
    if value.is_pointer_value() {
        return compiler.build_load(value.into_pointer_value(), "0");
    }
    return value.clone();
}

fn build_cast(first: &BasicValueEnum, _second: BasicTypeEnum, compiler: &CompilerImpl) {
    //TODO float casting
    compiler.builder.build_return(Some(&compiler.builder.build_load(first.into_pointer_value(), "1")));
}