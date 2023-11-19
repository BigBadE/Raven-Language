use crate::compiler::CompilerImpl;
use crate::internal::instructions::malloc_type;
use crate::type_getter::CompilerTypeGetter;
use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::{AddressSpace, IntPredicate};

pub fn math_internal<'ctx>(
    type_getter: &CompilerTypeGetter<'ctx>,
    compiler: &CompilerImpl<'ctx>,
    name: &String,
    value: &FunctionValue<'ctx>,
) -> bool {
    let params = value.get_params();
    if name.starts_with("math::Add") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);

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
    } else if name.starts_with("math::Subtract") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);
        let returning = compiler.builder.build_int_sub(
            compiler
                .builder
                .build_load(params.first().unwrap().into_pointer_value(), "2")
                .into_int_value(),
            compiler
                .builder
                .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                .into_int_value(),
            "1",
        );
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::Multiply") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);
        let returning = compiler.builder.build_int_mul(
            compiler
                .builder
                .build_load(params.first().unwrap().into_pointer_value(), "2")
                .into_int_value(),
            compiler
                .builder
                .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                .into_int_value(),
            "1",
        );
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::Divide") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);
        let returning = if name.ends_with("u64") {
            compiler.builder.build_int_unsigned_div(
                compiler
                    .builder
                    .build_load(params.first().unwrap().into_pointer_value(), "2")
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                    .into_int_value(),
                "1",
            )
        } else {
            compiler.builder.build_int_signed_div(
                compiler
                    .builder
                    .build_load(params.first().unwrap().into_pointer_value(), "2")
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                    .into_int_value(),
                "1",
            )
        };
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::Remainder") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);
        let returning = if name.ends_with("u64") {
            compiler.builder.build_int_unsigned_rem(
                compiler
                    .builder
                    .build_load(params.first().unwrap().into_pointer_value(), "2")
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                    .into_int_value(),
                "1",
            )
        } else {
            compiler.builder.build_int_signed_rem(
                compiler
                    .builder
                    .build_load(params.first().unwrap().into_pointer_value(), "2")
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(params.get(1).unwrap().into_pointer_value(), "3")
                    .into_int_value(),
                "1",
            )
        };
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::Equal") {
        compile_relational_op(IntPredicate::EQ, compiler, &params, type_getter);
    } else if name.starts_with("math::GreaterThan") {
        if is_unsigned(name) {
            compile_relational_op(IntPredicate::UGT, compiler, &params, type_getter)
        } else {
            compile_relational_op(IntPredicate::SGT, compiler, &params, type_getter)
        };
    } else if name.starts_with("math::LessThan") {
        if is_unsigned(name) {
            compile_relational_op(IntPredicate::ULT, compiler, &params, type_getter)
        } else {
            compile_relational_op(IntPredicate::SLT, compiler, &params, type_getter)
        };
    } else if name.starts_with("math::Not") {
        let malloc = malloc_type(
            type_getter,
            type_getter
                .compiler
                .context
                .bool_type()
                .ptr_type(AddressSpace::default())
                .const_zero(),
            &mut 0,
        );
        let returning = compiler.builder.build_not(
            compiler
                .builder
                .build_load(params.first().unwrap().into_pointer_value(), "1")
                .into_int_value(),
            "0",
        );
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::BitInvert") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);
        let returning = compiler.builder.build_not(
            compiler
                .builder
                .build_load(params.first().unwrap().into_pointer_value(), "1")
                .into_int_value(),
            "0",
        );
        compiler.builder.build_store(malloc, returning);
        compiler.builder.build_return(Some(&malloc));
    } else if name.starts_with("math::BitXOR") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);

        let returning = compiler.builder.build_xor(
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
    } else if name.starts_with("math::BitOr") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);

        let returning = compiler.builder.build_or(
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
    } else if name.starts_with("math::BitAnd") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);

        let returning = compiler.builder.build_and(
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
    } else if name.starts_with("math::BitXOR") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, pointer_type.get_type().const_zero(), &mut 0);

        let returning = compiler.builder.build_xor(
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
    } else if name.starts_with("math::And") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(
            type_getter,
            type_getter
                .compiler
                .context
                .bool_type()
                .ptr_type(AddressSpace::default())
                .const_zero(),
            &mut 0,
        );

        let returning = compiler.builder.build_and(
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
    } else if name.starts_with("math::XOR") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(
            type_getter,
            type_getter
                .compiler
                .context
                .bool_type()
                .ptr_type(AddressSpace::default())
                .const_zero(),
            &mut 0,
        );

        let returning = compiler.builder.build_xor(
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
    } else if name.starts_with("math::Or") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(
            type_getter,
            type_getter
                .compiler
                .context
                .bool_type()
                .ptr_type(AddressSpace::default())
                .const_zero(),
            &mut 0,
        );

        let returning = compiler.builder.build_or(
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
    } else {
        return false;
    }
    return true;
}

fn compile_relational_op(
    op: IntPredicate,
    compiler: &CompilerImpl,
    params: &Vec<BasicValueEnum>,
    type_getter: &CompilerTypeGetter,
) {
    let malloc = malloc_type(
        type_getter,
        type_getter
            .compiler
            .context
            .bool_type()
            .ptr_type(AddressSpace::default())
            .const_zero(),
        &mut 0,
    );
    let returning = compiler.builder.build_int_compare(
        op,
        compiler
            .builder
            .build_load(params.first().unwrap().into_pointer_value(), "2")
            .into_int_value(),
        compiler
            .builder
            .build_load(params.get(1).unwrap().into_pointer_value(), "3")
            .into_int_value(),
        "1",
    );
    compiler.builder.build_store(malloc, returning);
    compiler.builder.build_return(Some(&malloc));
}

fn is_unsigned(name: &String) -> bool {
    if name.ends_with("u64")
        || name.ends_with("u32")
        || name.ends_with("u16")
        || name.ends_with("u8")
    {
        return true;
    }
    return false;
}
