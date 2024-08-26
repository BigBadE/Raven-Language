use crate::compiler::CompilerImpl;
use inkwell::builder::{Builder, BuilderError};
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};
use inkwell::IntPredicate;
use std::fmt::Debug;

/// Compiles internal math functions
pub fn math_internal<'ctx>(compiler: &CompilerImpl<'ctx>, name: &String, value: &FunctionValue<'ctx>) -> bool {
    let params = value.get_params();
    if name.starts_with("math::Add") {
        compile_two_arg_func(compiler, &params, &Builder::build_int_add);
    } else if name.starts_with("math::Subtract") {
        compile_two_arg_func(compiler, &params, &Builder::build_int_sub);
    } else if name.starts_with("math::Multiply") {
        compile_two_arg_func(compiler, &params, &Builder::build_int_mul);
    } else if name.starts_with("math::Divide") {
        if name.ends_with("u64") {
            compile_two_arg_func(compiler, &params, &Builder::build_int_unsigned_div);
        } else {
            compile_two_arg_func(compiler, &params, &Builder::build_int_signed_div);
        }
    } else if name.starts_with("math::Remainder") {
        if name.ends_with("u64") {
            compile_two_arg_func(compiler, &params, &Builder::build_int_unsigned_rem);
        } else {
            compile_two_arg_func(compiler, &params, &Builder::build_int_signed_rem);
        }
    } else if name.starts_with("math::Equal") {
        compile_relational_op(IntPredicate::EQ, compiler, &params);
    } else if name.starts_with("math::GreaterThan") {
        if is_unsigned(name) {
            compile_relational_op(IntPredicate::UGT, compiler, &params)
        } else {
            compile_relational_op(IntPredicate::SGT, compiler, &params)
        };
    } else if name.starts_with("math::LessThan") {
        if is_unsigned(name) {
            compile_relational_op(IntPredicate::ULT, compiler, &params)
        } else {
            compile_relational_op(IntPredicate::SLT, compiler, &params)
        };
    } else if name.starts_with("math::Not") || name.starts_with("math::BitInvert") {
        compile_one_arg_func(compiler, &params, &Builder::build_not);
    } else if name.starts_with("math::BitXOR") || name.starts_with("math::XOR") {
        compile_two_arg_func(compiler, &params, &Builder::build_xor);
    } else if name.starts_with("math::BitOr") || name.starts_with("math::Or") {
        compile_two_arg_func(compiler, &params, &Builder::build_or);
    } else if name.starts_with("math::BitAnd") || name.starts_with("math::And") {
        compile_two_arg_func(compiler, &params, &Builder::build_and);
    } else if name.starts_with("math::RightShift") {
        compile_two_arg_func(compiler, &params, &|builder, lhs, rhs, name| builder.build_right_shift(lhs, rhs, false, name));
    } else if name.starts_with("math::LogicRightShift") {
        compile_two_arg_func(compiler, &params, &|builder, lhs, rhs, name| builder.build_right_shift(lhs, rhs, true, name));
    } else if name.starts_with("math::LeftShift") {
        compile_two_arg_func(compiler, &params, &Builder::build_left_shift);
    } else {
        return false;
    }
    return true;
}

/// Creates a two-argument internal function, calling the function on both arguments
fn compile_two_arg_func<'ctx>(
    compiler: &CompilerImpl<'ctx>,
    params: &Vec<BasicValueEnum<'ctx>>,
    function: &dyn Fn(&Builder<'ctx>, IntValue<'ctx>, IntValue<'ctx>, &str) -> Result<IntValue<'ctx>, BuilderError>,
) {
    let returning = function(&compiler.builder, params[0].into_int_value(), params[1].into_int_value(), "1").unwrap();
    compiler.builder.build_return(Some(&returning)).unwrap();
}

/// Creates a one-argument internal function, calling the function on one argument
fn compile_one_arg_func<'ctx>(
    compiler: &CompilerImpl<'ctx>,
    params: &Vec<BasicValueEnum<'ctx>>,
    function: &dyn Fn(&Builder<'ctx>, IntValue<'ctx>, &str) -> Result<IntValue<'ctx>, BuilderError>,
) {
    let returning = function(&compiler.builder, params[0].into_int_value(), "1").unwrap();
    compiler.builder.build_return(Some(&returning)).unwrap();
}

/// Compiles relational operators
fn compile_relational_op(op: IntPredicate, compiler: &CompilerImpl, params: &Vec<BasicValueEnum>) {
    let returning =
        compiler.builder.build_int_compare(op, params[0].into_int_value(), params[1].into_int_value(), "1").unwrap();
    compiler.builder.build_return(Some(&returning)).unwrap();
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
