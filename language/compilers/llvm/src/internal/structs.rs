use inkwell::context::Context;
use inkwell::types::{BasicType, BasicTypeEnum};

pub fn get_internal_struct<'ctx>(
    context: &'ctx Context,
    name: &str,
) -> Option<BasicTypeEnum<'ctx>> {
    return match name {
        "i64" => Some(context.i64_type().as_basic_type_enum()),
        "i32" => Some(context.i32_type().as_basic_type_enum()),
        "i16" => Some(context.i16_type().as_basic_type_enum()),
        "i8" => Some(context.i8_type().as_basic_type_enum()),
        "u64" => Some(context.i64_type().as_basic_type_enum()),
        "u32" => Some(context.i32_type().as_basic_type_enum()),
        "u16" => Some(context.i16_type().as_basic_type_enum()),
        "u8" => Some(context.i8_type().as_basic_type_enum()),
        "bool" => Some(context.bool_type().as_basic_type_enum()),
        _ => None,
    };
}
