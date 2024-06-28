
use crate::compiler::CompilerImpl;
use crate::internal::instructions::malloc_type;
use crate::type_getter::CompilerTypeGetter;
use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::IntPredicate;

/// Compiles internal math functions
pub fn math_internal<'ctx>(
    type_getter: &CompilerTypeGetter<'ctx>,
    compiler: &CompilerImpl<'ctx>,
    name: &String,
    value: &FunctionValue<'ctx>,
) -> bool {
    let params = value.get_params();
    if name.starts_with("math::Add") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_int_add(
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
    } else if name.starts_with("math::Subtract") {
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);
        let returning = compiler
            .builder
            .build_int_sub(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
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
    } else if name.starts_with("math::Multiply") {
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);
        let returning = compiler
            .builder
            .build_int_mul(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
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
    } else if name.starts_with("math::Divide") {
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);
        let returning = if name.ends_with("u64") {
            compiler.builder.build_int_unsigned_div(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                "1",
            )
        } else {
            compiler.builder.build_int_signed_div(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                "1",
            )
        }
        .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
    } else if name.starts_with("math::Remainder") {
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);
        let returning = if name.ends_with("u64") {
            compiler.builder.build_int_unsigned_rem(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                "1",
            )
        } else {
            compiler.builder.build_int_signed_rem(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
                    .unwrap()
                    .into_int_value(),
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.get(1).unwrap().into_pointer_value(), "3")
                    .unwrap()
                    .into_int_value(),
                "1",
            )
        }
        .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
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
        let malloc = malloc_type(type_getter, type_getter.compiler.context.bool_type().size_of(), &mut 0);
        let returning = compiler
            .builder
            .build_not(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "1")
                    .unwrap()
                    .into_int_value(),
                "0",
            )
            .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
    } else if name.starts_with("math::BitInvert") {
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);
        let returning = compiler
            .builder
            .build_not(
                compiler
                    .builder
                    .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "1")
                    .unwrap()
                    .into_int_value(),
                "0",
            )
            .unwrap();
        compiler.builder.build_store(malloc, returning).unwrap();
        compiler.builder.build_return(Some(&malloc)).unwrap();
    } else if name.starts_with("math::BitXOR") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_xor(
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
    } else if name.starts_with("math::BitOr") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_or(
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
    } else if name.starts_with("math::BitAnd") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_and(
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
    } else if name.starts_with("math::BitXOR") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_xor(
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
    } else if name.starts_with("math::And") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, type_getter.compiler.context.bool_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_and(
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
    } else if name.starts_with("math::XOR") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, type_getter.compiler.context.bool_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_xor(
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
    } else if name.starts_with("math::Or") {
        let pointer_type = params.first().unwrap().into_pointer_value();
        let malloc = malloc_type(type_getter, type_getter.compiler.context.bool_type().size_of(), &mut 0);

        let returning = compiler
            .builder
            .build_or(
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
    } else {
        return false;
    }
    return true;
}

/// Compiles relational operators
fn compile_relational_op(
    op: IntPredicate,
    compiler: &CompilerImpl,
    params: &Vec<BasicValueEnum>,
    type_getter: &CompilerTypeGetter,
) {
    let malloc = malloc_type(type_getter, type_getter.compiler.context.bool_type().size_of(), &mut 0);

    let returning = compiler
        .builder
        .build_int_compare(
            op,
            compiler
                .builder
                .build_load(type_getter.compiler.context.i64_type(), params.first().unwrap().into_pointer_value(), "2")
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
}

/// Returns true if a number is unsigned
fn is_unsigned(name: &String) -> bool {
    if name.ends_with("u64") || name.ends_with("u32") || name.ends_with("u16") || name.ends_with("u8") {
        return true;
    }
    return false;
}
