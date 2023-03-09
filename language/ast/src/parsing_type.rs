use crate::type_resolver::TypeResolver;

pub trait ParsingType<T> {
    fn finalize(type_manager: &dyn TypeResolver) -> T;
}

pub struct ParsingTypes {

}

pub struct ParsingStruct {

}

pub struct ParsingFunction {

}