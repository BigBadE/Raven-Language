use std::collections::HashMap;
use std::rc::Rc;
use inkwell::types::BasicTypeEnum;
use inkwell::values::GlobalValue;
use syntax::types::Types;

pub struct ResolvedType<'ctx> {
    pub types: BasicTypeEnum<'ctx>,
    pub vtables: HashMap<Rc<Types>, GlobalValue<'ctx>>
}

impl<'ctx> ResolvedType<'ctx> {
    pub fn new(types: BasicTypeEnum<'ctx>, vtables: HashMap<Rc<Types>, GlobalValue<'ctx>>) -> Self{
        return Self {
            types,
            vtables
        }
    }
}