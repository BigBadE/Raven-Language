use std::collections::HashMap;
use std::sync::Arc;
use crate::fields::Field;
use crate::functions::Function;
use crate::types::{GenericType, Types, UnresolvedGenericType};

pub struct Structure<T> where T: Types {
    pub name: String,
    fields: HashMap<String, Arc<Field<T>>>,
    functions: HashMap<String, Arc<Function<T>>>,
    generics: HashMap<String, Arc<T>>
}

impl Structure<UnresolvedGenericType> {
    pub fn resolve(&self) -> Structure<GenericType> {
        todo!()
    }
}