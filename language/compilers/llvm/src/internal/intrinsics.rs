use std::sync::Arc;
use inkwell::AddressSpace;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::FunctionValue;
use syntax::function::CodelessFinalizedFunction;
use crate::type_getter::CompilerTypeGetter;

pub fn compile_llvm_intrinsics<'ctx>(function: &Arc<CodelessFinalizedFunction>, type_getter: &mut CompilerTypeGetter<'ctx>)
                                     -> FunctionValue<'ctx> {
    type_getter.compiler.module.add_function(&function.data.name, match function.data.name.as_str() {
        "printf" => type_getter.compiler.context.void_type().fn_type(&[BasicMetadataTypeEnum::from(type_getter.compiler.context.i32_type()),
            BasicMetadataTypeEnum::from(type_getter.compiler.context.i8_type().ptr_type(AddressSpace::default()))], true),
        _ => panic!("Tried to compile unknown LLVM intrinsic {}", function.data.name)
    }, None)
}