use std::fmt::{Display, Formatter};
use std::sync::Arc;
use crate::Struct;

pub enum Types {
    Struct(Arc<Struct>),
    Reference(Arc<Struct>)
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::Struct(structure) => write!(f, "{}", structure.name),
            Types::Reference(structure) => write!(f, "&{}", structure.name)
        }
    }
}