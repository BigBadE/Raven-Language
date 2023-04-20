use crate::types::Types;

pub struct Field<T> where T: Types {
    name: String,
    types: T
}