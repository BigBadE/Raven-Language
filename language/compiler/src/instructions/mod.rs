use inkwell::IntPredicate;
use inkwell::values::FunctionValue;
use ast::code::Field;
use crate::compiler::Compiler;

pub fn compile_internal<'ctx>(compiler: &Compiler<'ctx>, name: &String, fields: &Vec<Field>, value: FunctionValue<'ctx>) {
    let block = compiler.context.append_basic_block(value, "0");
    compiler.builder.position_at_end(block);
    let params = value.get_params();
    if fields.len() == 2 {
        match name.as_str() {
            "math::+" => {
                if check_types(fields, vec!("i64", "i64")) {
                    let returning = compiler.builder.build_int_add(params.get(0).unwrap().into_int_value(), params.get(1).unwrap().into_int_value(), "1");
                    compiler.builder.build_return(Some(&returning));
                }
            }
            "math::-" => {
                if check_types(fields, vec!("i64", "i64")) {
                    let returning = compiler.builder.build_int_sub(params.get(0).unwrap().into_int_value(), params.get(1).unwrap().into_int_value(), "1");
                    compiler.builder.build_return(Some(&returning));
                }
            }
            "math::/" => {
                if check_types(fields, vec!("i64", "i64")) {
                    let returning = compiler.builder.build_int_signed_div(params.get(0).unwrap().into_int_value(), params.get(1).unwrap().into_int_value(), "1");
                    compiler.builder.build_return(Some(&returning));
                }
            }
            "math::*" => {
                if check_types(fields, vec!("i64", "i64")) {
                    let returning = compiler.builder.build_int_mul(params.get(0).unwrap().into_int_value(), params.get(1).unwrap().into_int_value(), "1");
                    compiler.builder.build_return(Some(&returning));
                }
            }
            "math::==" => {
                if check_types(fields, vec!("i64", "i64")) {
                    let returning = compiler.builder
                        .build_int_compare(IntPredicate::EQ, params.get(0).unwrap().into_int_value(),
                                           params.get(1).unwrap().into_int_value(), "1");
                    compiler.builder.build_return(Some(&returning));
                }
            }
            _ => panic!("Unknown internal operation: {}", name)
        }
    }
}

fn check_types(fields: &Vec<Field>, checking: Vec<&str>) -> bool {
    for i in 0..fields.len() {
        if &fields.get(i).unwrap().field_type.name != checking.get(i).unwrap() {
            return false;
        }
    }
    return true;
}