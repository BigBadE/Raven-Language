use inkwell::AddressSpace;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::FunctionValue;
use crate::type_getter::CompilerTypeGetter;

pub fn compile_llvm_intrinsics<'ctx>(name: &str, type_getter: &CompilerTypeGetter<'ctx>)
                                     -> FunctionValue<'ctx> {
    type_getter.compiler.module.add_function(&name, match name {
        "printf" => type_getter.compiler.context.i32_type().fn_type(&[
            BasicMetadataTypeEnum::from(type_getter.compiler.context.i8_type().ptr_type(AddressSpace::default()))], true),
        "malloc" => type_getter.compiler.context.i8_type().ptr_type(AddressSpace::default()).fn_type(&[
            BasicMetadataTypeEnum::from(type_getter.compiler.context.i64_type().ptr_type(AddressSpace::default()))], false),
        _ => panic!("Tried to compile unknown LLVM intrinsic {}", name)
    }, None)
}