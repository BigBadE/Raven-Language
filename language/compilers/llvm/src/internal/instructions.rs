use inkwell::IntPredicate;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use crate::compiler::CompilerImpl;

pub fn compile_internal<'ctx>(compiler: &CompilerImpl<'ctx>, name: &String, value: FunctionValue<'ctx>) {
    let block = compiler.context.append_basic_block(value, "0");
    compiler.builder.position_at_end(block);
    let params = value.get_params();
    //Trunc to go u64 -> u8
    if name.starts_with("math::cast_") {
        build_cast(value.get_params().get(0).unwrap(), value.get_type().get_return_type().unwrap(), compiler);
        return;
    }
    match name.as_str() {
        "math::{}+{}" => {
            let returning = compiler.builder.build_int_add(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                           compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}-{}" => {
            let returning = compiler.builder.build_int_sub(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                           compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}/{}" => {
            let returning = compiler.builder.build_int_signed_div(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                           compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}*{}" => {
            let returning = compiler.builder.build_int_mul(compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                                           compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}=={}" => {
            let returning = compiler.builder
                .build_int_compare(IntPredicate::EQ, compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                   compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}!={}" => {
            let returning = compiler.builder
                .build_int_compare(IntPredicate::NE, compiler.builder.build_load(params.get(0).unwrap().into_pointer_value(), "2").into_int_value(),
                                   compiler.builder.build_load(params.get(1).unwrap().into_pointer_value(), "3").into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}>={}" => {
            let returning = compiler.builder
                .build_int_compare(IntPredicate::SGE, params.get(0).unwrap().into_int_value(),
                                   params.get(1).unwrap().into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}<={}" => {
            let returning = compiler.builder
                .build_int_compare(IntPredicate::SLE, params.get(0).unwrap().into_int_value(),
                                   params.get(1).unwrap().into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}<{}" => {
            let returning = compiler.builder
                .build_int_compare(IntPredicate::SLT, params.get(0).unwrap().into_int_value(),
                                   params.get(1).unwrap().into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}>{}" => {
            let returning = compiler.builder
                .build_int_compare(IntPredicate::SGT, params.get(0).unwrap().into_int_value(),
                                   params.get(1).unwrap().into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}&&{}" => {
            let returning = compiler.builder.build_and(params.get(0).unwrap().into_int_value(),
                                                       params.get(1).unwrap().into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        "math::{}||{}" => {
            let returning = compiler.builder.build_or(params.get(0).unwrap().into_int_value(),
                                                      params.get(1).unwrap().into_int_value(), "1");
            compiler.builder.build_return(Some(&returning));
        }
        _ => panic!("Unknown internal operation: {}", name)
    }
}

fn build_cast(first: &BasicValueEnum, second: BasicTypeEnum, compiler: &CompilerImpl) {
    //TODO float casting
    compiler.builder.build_return(Some(&compiler.builder.build_load(first.into_pointer_value(), "1")));
}