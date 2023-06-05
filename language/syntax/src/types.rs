use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use async_recursion::async_recursion;
use crate::function::{display, display_parenless};
use crate::{is_modifier, Modifier, ParsingError, Struct};
use crate::async_getters::ImplementationGetter;
use crate::code::MemberField;
use crate::syntax::{ParsingType, Syntax};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    pub fn id(&self) -> u64 {
        return match self {
            Types::Struct(structure) => structure.id,
            Types::Reference(inner) => inner.id(),
            _ => panic!("Tried to ID generic!")
        };
    }

    pub fn is_primitive(&self) -> bool {
        return match self {
            Types::Struct(structure) => is_modifier(structure.modifiers, Modifier::Internal),
            Types::Reference(inner) => inner.is_primitive(),
            _ => panic!("Tried to primitive check a generic!")
        };
    }

    pub fn get_fields(&self) -> &Vec<ParsingType<MemberField>> {
        return match self {
            Types::Struct(structure) => &structure.fields,
            Types::Reference(inner) => inner.get_fields(),
            Types::GenericType(base, _) => base.get_fields(),
            Types::Generic(_, _) => panic!("Tried to get fields of generic!")
        };
    }

    #[async_recursion]
    pub async fn of_type(&self, other: &Types, syntax: &Arc<Mutex<Syntax>>) -> bool {
        return match self {
            Types::Struct(found) => match other {
                Types::Struct(other_struct) => found == other_struct ||
                    ImplementationGetter::new(syntax.clone(), self.clone(), other.clone()).await.is_ok(),
                Types::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound, syntax).await {
                            return false;
                        }
                    }
                    true
                }
                Types::GenericType(base, _) => self.of_type(base, syntax).await,
                _ => false
            },
            Types::GenericType(base, _generics) => match other {
                Types::GenericType(_other_base, _other_generics) => {
                    if !base.of_type(self, syntax).await {
                        return false;
                    }

                    //TODO check generics, I have no clue how to with respect to subtypes.
                    todo!()
                }
                Types::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound, syntax).await {
                            return false;
                        }
                    }
                    true
                }
                _ => false
            }
            Types::Reference(referencing) => match other {
                Types::Reference(other) => referencing.of_type(other, syntax).await,
                _ => false
            },
            Types::Generic(_, bounds) => match other {
                Types::Generic(_, other_bounds) => {
                    'outer: for bound in bounds {
                        for other_bound in other_bounds {
                            if other_bound.of_type(bound, syntax).await {
                                continue 'outer;
                            }
                        }
                        return false;
                    }
                    true
                }
                _ => other.of_type(self, syntax).await
            }
        };
    }

    pub async fn resolve_generic(&self, other: &Types, syntax: &Arc<Mutex<Syntax>>,
                                 bounds_error: ParsingError) -> Result<Option<Types>, ParsingError> {
        match self {
            Types::Generic(_name, bounds) => {
                for bound in bounds {
                    if !other.of_type(bound, syntax).await {
                        return Err(bounds_error);
                    }
                }
                return Ok(Some(self.clone()));
            }
            _ => {}
        }
        return Ok(None);
    }

    pub async fn degeneric(&mut self, generics: &HashMap<String, Types>, syntax: &Arc<Mutex<Syntax>>,
                           none_error: ParsingError, bounds_error: ParsingError) -> Result<(), ParsingError> {
        match self {
            Types::Generic(name, bounds) => {
                return if let Some(found) = generics.get(name) {
                    for bound in bounds {
                        if !found.of_type(bound, syntax).await {
                            return Err(bounds_error);
                        }
                    }
                    *self = found.clone();
                    Ok(())
                } else {
                    println!("Failed to find {} in {:?}", name, generics.keys());
                    Err(none_error)
                };
            }
            _ => {}
        }
        return Ok(());
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

    #[async_recursion]
    pub async fn flatten(&mut self, generics: &mut Vec<Types>, syntax: &Arc<Mutex<Syntax>>) -> Result<Types, ParsingError> {
        for generic in &mut *generics {
            if let Types::GenericType(base, bounds) = generic {
                *generic = base.flatten(bounds, syntax).await?;
            }
        }
        return match self {
            Types::Struct(found) => {
                if generics.is_empty() {
                    return Ok(self.clone());
                }
                let name = format!("{}<{}>", found.name, display_parenless(generics, "_"));
                if syntax.lock().unwrap().structures.types.contains_key(&name) {
                    Ok(Types::Struct(syntax.lock().unwrap().structures.types.get(&name).unwrap().clone()))
                } else {
                    let mut other = Struct::clone(found);
                    other.degeneric(generics, syntax).await?;
                    other.name = name.clone();
                    let other = Arc::new(other);
                    syntax.lock().unwrap().structures.types.insert(name, other.clone());
                    Ok(Types::Struct(other))
                }
            }
            Types::Reference(other) => other.flatten(generics, syntax).await,
            Types::Generic(_, _) => panic!("Unresolved generic!"),
            Types::GenericType(base, effects) =>
                base.flatten(effects, syntax).await
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
                write!(f, "{}<{}>", types, display_parenless(generics, "_"))
        }
    }
}