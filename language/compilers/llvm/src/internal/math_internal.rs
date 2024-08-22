use crate::compiler::CompilerImpl;
use crate::internal::instructions::malloc_type;
use crate::type_getter::CompilerTypeGetter;
use inkwell::builder::{Builder, BuilderError};
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};
use inkwell::IntPredicate;

/// Compiles internal math functions
pub fn math_internal<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    compiler: &CompilerImpl<'ctx>,
    name: &String,
    value: &FunctionValue<'ctx>,
) -> bool {
    let params = value.get_params();
    if name.starts_with("math::Add") {
        compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_add);
    } else if name.starts_with("math::Subtract") {
        compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_sub);
    } else if name.starts_with("math::Multiply") {
        compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_mul);
    } else if name.starts_with("math::Divide") {
        if name.ends_with("u64") {
            compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_unsigned_div);
        } else {
            compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_signed_div);
        }
    } else if name.starts_with("math::Remainder") {
        if name.ends_with("u64") {
            compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_unsigned_rem);
        } else {
            compile_two_arg_func(type_getter, compiler, &params, &Builder::build_int_signed_rem);
        }
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
    } else if name.starts_with("math::Not") || name.starts_with("math::BitInvert") {
        compile_one_arg_func(type_getter, compiler, &params, &Builder::build_not);
    } else if name.starts_with("math::BitXOR") || name.starts_with("math::XOR") {
        compile_two_arg_func(type_getter, compiler, &params, &Builder::build_xor);
    } else if name.starts_with("math::BitOr") || name.starts_with("math::Or") {
        compile_two_arg_func(type_getter, compiler, &params, &Builder::build_or);
    } else if name.starts_with("math::BitAnd") || name.starts_with("math::And") {
        compile_two_arg_func(type_getter, compiler, &params, &Builder::build_and);
    } else {
        return false;
    }
    return true;
}

/// Creates a two-argument internal function, calling the function on both arguments
fn compile_two_arg_func<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    compiler: &CompilerImpl<'ctx>,
    params: &Vec<BasicValueEnum<'ctx>>,
    function: &dyn Fn(&Builder<'ctx>, IntValue<'ctx>, IntValue<'ctx>, &str) -> Result<IntValue<'ctx>, BuilderError>,
) {
    let pointer_type = params.first().unwrap().into_pointer_value();
    let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of());

    let returning = function(
        &compiler.builder,
        compiler.builder.build_load(type_getter.compiler.context.i64_type(), pointer_type, "2").unwrap().into_int_value(),
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

/// Creates a one-argument internal function, calling the function on one argument
fn compile_one_arg_func<'ctx>(
    type_getter: &mut CompilerTypeGetter<'ctx>,
    compiler: &CompilerImpl<'ctx>,
    params: &Vec<BasicValueEnum<'ctx>>,
    function: &dyn Fn(&Builder<'ctx>, IntValue<'ctx>, &str) -> Result<IntValue<'ctx>, BuilderError>,
) {
    let pointer_type = params.first().unwrap().into_pointer_value();
    let malloc = malloc_type(type_getter, compiler.context.i64_type().size_of());

    let returning = function(
        &compiler.builder,
        compiler.builder.build_load(type_getter.compiler.context.i64_type(), pointer_type, "2").unwrap().into_int_value(),
        "1",
    )
    .unwrap();
    compiler.builder.build_store(malloc, returning).unwrap();
    compiler.builder.build_return(Some(&malloc)).unwrap();
}

/// Compiles relational operators
fn compile_relational_op(
    op: IntPredicate,
    compiler: &CompilerImpl,
    params: &Vec<BasicValueEnum>,
    type_getter: &mut CompilerTypeGetter,
) {
    let malloc = malloc_type(type_getter, type_getter.compiler.context.bool_type().size_of());

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
    return match name {
        _ if name.ends_with("u64") => true,
        _ if name.ends_with("u32") => true,
        _ if name.ends_with("u16") => true,
        _ if name.ends_with("u8") => true,
        _ => false,
    };
}
