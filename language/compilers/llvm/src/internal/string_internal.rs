use crate::compiler::CompilerImpl;
use crate::internal::instructions::malloc_type;
use crate::internal::intrinsics::compile_llvm_intrinsics;
use crate::type_getter::CompilerTypeGetter;
use inkwell::values::{BasicMetadataValueEnum, BasicValue, FunctionValue};
use inkwell::AddressSpace;

pub fn string_internal<'ctx>(
    type_getter: &CompilerTypeGetter<'ctx>,
    compiler: &CompilerImpl<'ctx>,
    name: &String,
    value: &FunctionValue<'ctx>,
) -> bool {
    let params = value.get_params();
    if name.starts_with("string::Cast") {
        type_getter
            .compiler
            .builder
            .build_return(Some(value.get_params().first().unwrap()));
    } else if name.starts_with("string::Add<char + u64>_char::add") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);
        let pointer_type = compiler
            .builder
            .build_bitcast(
                pointer_type,
                compiler
                    .context
                    .i64_type()
                    .ptr_type(AddressSpace::default()),
                "1",
            )
            .into_pointer_value();
        let returning = compiler.builder.build_int_add(
            compiler
                .builder
                .build_load(pointer_type, "2")
                .into_int_value(),
            compiler
                .builder
                .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                .into_int_value(),
            "1",
        );
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("string::Add<str + str>_str::add") {
        let length = type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("strlen")
                    .unwrap_or_else(|| compile_llvm_intrinsics("strlen", type_getter)),
                &[BasicMetadataValueEnum::PointerValue(
                    value.get_params().first().unwrap().into_pointer_value(),
                )],
                "0",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_int_value();
        let second_length = type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("strlen")
                    .unwrap_or_else(|| compile_llvm_intrinsics("strlen", type_getter)),
                &[BasicMetadataValueEnum::PointerValue(
                    value.get_params().get(1).unwrap().into_pointer_value(),
                )],
                "2",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_int_value();
        let total = type_getter
            .compiler
            .builder
            .build_int_add(length, second_length, "4");
        let malloc = type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("malloc")
                    .unwrap_or_else(|| compile_llvm_intrinsics("malloc", type_getter)),
                &[BasicMetadataValueEnum::IntValue(total)],
                "5",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_pointer_value();
        type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("strcpy")
                    .unwrap_or_else(|| compile_llvm_intrinsics("strcpy", type_getter)),
                &[
                    BasicMetadataValueEnum::PointerValue(malloc),
                    BasicMetadataValueEnum::PointerValue(
                        value.get_params().first().unwrap().into_pointer_value(),
                    ),
                ],
                "6",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_pointer_value();
        type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("strcat")
                    .unwrap_or_else(|| compile_llvm_intrinsics("strcat", type_getter)),
                &[
                    BasicMetadataValueEnum::PointerValue(malloc),
                    BasicMetadataValueEnum::PointerValue(
                        value.get_params().get(1).unwrap().into_pointer_value(),
                    ),
                ],
                "7",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_pointer_value();
        type_getter
            .compiler
            .builder
            .build_return(Some(&malloc.as_basic_value_enum()));
    } else if name.starts_with("string::Add<str + char>_str::add") {
        let length = type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("strlen")
                    .unwrap_or_else(|| compile_llvm_intrinsics("strlen", type_getter)),
                &[BasicMetadataValueEnum::PointerValue(
                    value.get_params().first().unwrap().into_pointer_value(),
                )],
                "0",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_int_value();
        let total = type_getter.compiler.builder.build_int_add(
            length,
            type_getter.compiler.context.i64_type().const_int(1, false),
            "4",
        );
        let malloc = type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("malloc")
                    .unwrap_or_else(|| compile_llvm_intrinsics("malloc", type_getter)),
                &[BasicMetadataValueEnum::IntValue(total)],
                "5",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_pointer_value();
        type_getter
            .compiler
            .builder
            .build_call(
                type_getter
                    .compiler
                    .module
                    .get_function("strcpy")
                    .unwrap_or_else(|| compile_llvm_intrinsics("strcpy", type_getter)),
                &[
                    BasicMetadataValueEnum::PointerValue(malloc),
                    BasicMetadataValueEnum::PointerValue(
                        value.get_params().first().unwrap().into_pointer_value(),
                    ),
                ],
                "6",
            )
            .try_as_basic_value()
            .unwrap_left()
            .into_pointer_value();
        type_getter.compiler.builder.build_store(
            unsafe {
                type_getter
                    .compiler
                    .builder
                    .build_in_bounds_gep(malloc, &[length], "7")
            },
            type_getter
                .compiler
                .builder
                .build_load(value.get_params().get(1).unwrap().into_pointer_value(), "8"),
        );
        let plus_one = type_getter.compiler.builder.build_int_add(
            length,
            type_getter.compiler.context.i64_type().const_int(1, false),
            "9",
        );
        type_getter.compiler.builder.build_store(
            unsafe {
                type_getter
                    .compiler
                    .builder
                    .build_in_bounds_gep(malloc, &[plus_one], "10")
            },
            type_getter.compiler.context.i8_type().const_zero(),
        );
        type_getter
            .compiler
            .builder
            .build_return(Some(&malloc.as_basic_value_enum()));
    } else {
        return false;
    }
    return true;
}
