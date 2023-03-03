use crate::type_resolver::TypeResolver;

pub trait CompilerInfo<'ctx> {
    fn finalize_types(&mut self, type_manager: &dyn TypeResolver<'ctx>);
}