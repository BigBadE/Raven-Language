use inkwell::values::BasicValueEnum;
use ast::code::MathOperator;
use crate::compiler::Compiler;

pub fn math_operation<'ctx>(operation: MathOperator, compiler: &Compiler<'ctx>,
                            first: Option<BasicValueEnum<'ctx>>, second: BasicValueEnum<'ctx>) -> BasicValueEnum<'ctx> {
    return match first {
        None => match second.get_type() {
            _ => panic!("Can't do a meth operation on this type!")
        },
        Some(first) => match first {
            BasicValueEnum::IntValue(first) => match second {
                BasicValueEnum::IntValue(second) => BasicValueEnum::IntValue(match operation {
                    MathOperator::PLUS => compiler.builder.build_int_add(first, second, "wtf?"),
                    _ => todo!()
                }),
                BasicValueEnum::StructValue(_struct_type) => todo!(),
                BasicValueEnum::FloatValue(_float) => todo!(),
                BasicValueEnum::PointerValue(_pointer) => todo!(),
                _ => panic!("Can't do a math operation on this type!")
            }
            BasicValueEnum::StructValue(_struct_type) => todo!(),
            BasicValueEnum::FloatValue(_float) => todo!(),
            BasicValueEnum::PointerValue(_pointer) => todo!(),
            _ => panic!("Can't do a math operation on this type!")
        }
    };
}