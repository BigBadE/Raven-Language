use crate::type_getter::CompilerTypeGetter;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;

pub fn compile_llvm_intrinsics<'ctx>(
    name: &str,
    type_getter: &CompilerTypeGetter<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(func) = type_getter.compiler.module.get_function(&name) {
        return func;
    }
    return type_getter.compiler.module.add_function(
        &name,
        match name {
            "printf" => type_getter.compiler.context.i32_type().fn_type(
                &[BasicMetadataTypeEnum::from(
                    type_getter
                        .compiler
                        .context
                        .i8_type()
                        .ptr_type(AddressSpace::default()),
                )],
                true,
            ),
            "malloc" => type_getter
                .compiler
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .fn_type(
                    &[BasicMetadataTypeEnum::from(
                        type_getter
                            .compiler
                            .context
                            .i64_type()
                            .ptr_type(AddressSpace::default()),
                    )],
                    false,
                ),
            "strcat" => type_getter
                .compiler
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .fn_type(
                    &[
                        BasicMetadataTypeEnum::from(
                            type_getter
                                .compiler
                                .context
                                .i8_type()
                                .ptr_type(AddressSpace::default()),
                        ),
                        BasicMetadataTypeEnum::from(
                            type_getter
                                .compiler
                                .context
                                .i8_type()
                                .ptr_type(AddressSpace::default()),
                        ),
                    ],
                    false,
                ),
            "strcpy" => type_getter
                .compiler
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .fn_type(
                    &[
                        BasicMetadataTypeEnum::from(
                            type_getter
                                .compiler
                                .context
                                .i8_type()
                                .ptr_type(AddressSpace::default()),
                        ),
                        BasicMetadataTypeEnum::from(
                            type_getter
                                .compiler
                                .context
                                .i8_type()
                                .ptr_type(AddressSpace::default()),
                        ),
                    ],
                    false,
                ),
            "strlen" => type_getter.compiler.context.i64_type().fn_type(
                &[BasicMetadataTypeEnum::from(
                    type_getter
                        .compiler
                        .context
                        .i8_type()
                        .ptr_type(AddressSpace::default()),
                )],
                false,
            ),
            "strcmp" => type_getter.compiler.context.i64_type().fn_type(
                &[
                    BasicMetadataTypeEnum::from(
                        type_getter
                            .compiler
                            .context
                            .i8_type()
                            .ptr_type(AddressSpace::default()),
                    ),
                    BasicMetadataTypeEnum::from(
                        type_getter
                            .compiler
                            .context
                            .i8_type()
                            .ptr_type(AddressSpace::default()),
                    ),
                ],
                false,
            ),
            _ => panic!("Tried to compile unknown LLVM intrinsic {}", name),
        },
        None,
    );
}
