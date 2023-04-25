use std::fmt::{Display, Formatter};
use std::sync::Arc;
use crate::function::display;
use crate::Struct;

#[derive(Clone, Debug)]
pub enum Types {
    //A basic struct
    Struct(Arc<Struct>),
    //A type with generic types
    GenericType(Box<Types>, Vec<Types>),
    //A reference to a type
    Reference(Box<Types>),
    //A generic with bounds
    Generic(String, Vec<Types>),
}

impl Types {
    pub fn of_type(&self, other: &Types) -> bool {
        return match self {
            Types::Struct(found) => match other {
                Types::Struct(other) => found == other,
                Types::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound) {
                            return false;
                        }
                    }
                    true
                },
                Types::GenericType(base, _) => self.of_type(base),
                _ => false
            },
            Types::GenericType(base, generics) => match other {
                Types::GenericType(other_base, other_generics) => {
                    if !base.of_type(self) {
                        return false;
                    }

                    //TODO check generics, I have no clue how to with respect to subtypes.
                    true
                },
                Types::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound)  {
                            return false;
                        }
                    }
                    true
                },
                _ => false
            }
            Types::Reference(referencing) => match other {
                Types::Reference(other) => referencing.of_type(other),
                _ => false
            },
            Types::Generic(_, bounds) => match other {
                Types::Generic(_, other_bounds) => {
                    'outer: for bound in bounds {
                        for other_bound in other_bounds {
                            if other_bound.of_type(bound) {
                                continue 'outer
                            }
                        }
                        return false;
                    }
                    true
                },
                _ => other.of_type(self)
            }
        }
    }

    pub fn clone_struct(&self) -> Arc<Struct> {
        //Must be cloned so Arcs can be gotten mutably. See Arc::get_mut_unchecked.
        return match self {
            Types::Struct(structs) => structs.clone(),
            Types::Reference(structs) => structs.clone_struct(),
            Types::GenericType(_, _) => panic!("Generics should never be clone'd into structs!"),
            Types::Generic(_, _) => panic!("Generics should never be clone'd into structs!")
        };
    }

    pub fn name(&self) -> String {
        return match self {
            Types::Struct(structs) => structs.name.clone(),
            Types::Reference(structs) => structs.name(),
            Types::Generic(_, _) => panic!("Generics should never be named"),
            Types::GenericType(_, _) => panic!("Generics should never be named")
        };
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::Struct(structure) => write!(f, "{}", structure.name),
            Types::Reference(structure) => write!(f, "&{}", structure),
            Types::Generic(name, bounds) =>
                write!(f, "{}: {}", name, display(bounds, " + ")),
            Types::GenericType(types, generics) =>
                write!(f, "{}<{}>", types, display(generics, ", "))
        }
    }
}