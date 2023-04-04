use std::fmt::{Display, Formatter};
use std::sync::Arc;
use crate::Struct;

#[derive(Clone)]
pub enum Types {
    Struct(Arc<Struct>),
    Reference(Arc<Struct>)
}

impl Types {
    pub fn into(&self) -> Arc<Struct> {
        //Must be cloned so Arcs can be gotten mutably. See Arc::get_mut_unchecked.
        return match self {
            Types::Struct(structs) => structs,
            Types::Reference(structs) => structs
        }.clone();
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::Struct(structure) => write!(f, "{}", structure.name),
            Types::Reference(structure) => write!(f, "&{}", structure.name)
        }
    }
}