use std::fmt::{Display, Formatter};
use std::sync::Arc;
use crate::function::display;
use crate::Struct;

#[derive(Clone)]
pub enum Types {
    Struct(Arc<Struct>),
    Reference(Box<Types>),
    Generic(String, Vec<Types>)
}

impl Types {
    pub fn clone_struct(&self) -> Arc<Struct> {
        //Must be cloned so Arcs can be gotten mutably. See Arc::get_mut_unchecked.
        return match self {
            Types::Struct(structs) => structs.clone(),
            Types::Reference(structs) => structs.clone_struct(),
            Types::Generic(_, _) => panic!("Generics should never be clone'd into structs!")
        };
    }
    
    pub fn name(&self) -> String {
        return match self {
            Types::Struct(structs) => structs.name.clone(),
            Types::Reference(structs) => structs.name(),
            Types::Generic(name, _) => name.clone()
        }
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::Struct(structure) => write!(f, "{}", structure.name),
            Types::Reference(structure) => write!(f, "&{}", structure),
            Types::Generic(name, bounds) => write!(f, "{}: {}", name, display(bounds, " + "))
        }
    }
}