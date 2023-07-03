use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use async_recursion::async_recursion;
use crate::function::{display, display_parenless};
use crate::{is_modifier, Modifier, ParsingError, StructData};
use crate::async_getters::ImplementationGetter;
use crate::async_util::AsyncDataGetter;
use crate::code::FinalizedMemberField;
use crate::r#struct::FinalizedStruct;
use crate::syntax::Syntax;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Types {
    //A basic struct
    Struct(Arc<StructData>),
    //A type with generic types
    GenericType(Box<Types>, Vec<Types>),
    //A reference to a type
    Reference(Box<Types>),
    //A generic with bounds
    Generic(String, Vec<Types>),
}

#[derive(Clone, Debug, Eq, Hash)]
pub enum FinalizedTypes {
    //A basic struct
    Struct(Arc<FinalizedStruct>),
    //A type with generic types
    GenericType(Box<FinalizedTypes>, Vec<FinalizedTypes>),
    //A reference to a type
    Reference(Box<FinalizedTypes>),
    //A generic with bounds
    Generic(String, Vec<FinalizedTypes>),
}

impl Types {
    pub fn name(&self) -> String {
        return match self {
            Types::Struct(structs) => structs.name.clone(),
            Types::Reference(structs) => structs.name(),
            Types::Generic(_, _) => panic!("Generics should never be named"),
            Types::GenericType(_, _) => panic!("Generics should never be named")
        };
    }

    #[async_recursion]
    pub async fn finalize(&self, syntax: Arc<Mutex<Syntax>>) -> FinalizedTypes {
        return match self {
            Types::Struct(structs) => FinalizedTypes::Struct(AsyncDataGetter::new(syntax, structs.clone()).await),
            Types::Reference(structs) => FinalizedTypes::Reference(Box::new(structs.finalize(syntax).await)),
            Types::Generic(name, bounds) => FinalizedTypes::Generic(name.clone(),
                                                                    Self::finalize_all(syntax, bounds).await),
            Types::GenericType(base, bounds) => FinalizedTypes::GenericType(Box::new(base.finalize(syntax.clone()).await),
                                                                        Self::finalize_all(syntax, bounds).await)
        };
    }

    async fn finalize_all(syntax: Arc<Mutex<Syntax>>, types: &Vec<Types>) -> Vec<FinalizedTypes> {
        let mut output = Vec::new();
        for found in types {
            output.push(found.finalize(syntax.clone()).await);
        }
        return output;
    }
}

impl FinalizedTypes {
    pub fn id(&self) -> u64 {
        return match self {
            FinalizedTypes::Struct(structure) => structure.data.id,
            FinalizedTypes::Reference(inner) => inner.id(),
            _ => panic!("Tried to ID generic!")
        };
    }

    pub fn get_fields(&self) -> &Vec<FinalizedMemberField> {
        return match self {
            FinalizedTypes::Struct(inner) => &inner.fields,
            FinalizedTypes::Reference(inner) => inner.get_fields(),
            _ => panic!("Tried to get fields of generic!")
        }
    }

    pub fn inner_struct(&self) -> &Arc<FinalizedStruct> {
        return match self {
            FinalizedTypes::Struct(structure) => structure,
            FinalizedTypes::Reference(inner) => inner.inner_struct(),
            _ => panic!("Tried to get inner struct of invalid type!")
        };
    }

    pub fn is_primitive(&self) -> bool {
        return match self {
            FinalizedTypes::Struct(structure) => is_modifier(structure.data.modifiers, Modifier::Internal) &&
                !is_modifier(structure.data.modifiers, Modifier::Extern),
            FinalizedTypes::Reference(_) => false,
            _ => panic!("Tried to primitive check a generic!")
        };
    }

    pub fn downcast(&self) -> Types {
        return match self {
            FinalizedTypes::Struct(inner) => Types::Struct(inner.data.clone()),
            FinalizedTypes::Reference(inner) => inner.downcast(),
            FinalizedTypes::Generic(name, bounds) => Types::Generic(name.clone(),
                                                                    bounds.iter().map(|bound| bound.downcast()).collect::<Vec<_>>()),
            FinalizedTypes::GenericType(base, bounds) =>
                Types::GenericType(Box::new(base.downcast()),
                                   bounds.iter().map(|bound| bound.downcast()).collect::<Vec<_>>())
        };
    }
    #[async_recursion]
    pub async fn of_type(&self, other: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>) -> bool {
        let output = match self {
            FinalizedTypes::Struct(found) => match other {
                FinalizedTypes::Struct(other_struct) =>
                    found == other_struct ||
                        ImplementationGetter::new(syntax.clone(), self.clone(), other.clone()).await.is_ok(),
                FinalizedTypes::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound, syntax).await {
                            return false;
                        }
                    }
                    true
                }
                FinalizedTypes::GenericType(base, _) => self.of_type(base, syntax).await,
                _ => false
            },
            FinalizedTypes::GenericType(base, _generics) => match other {
                FinalizedTypes::GenericType(_other_base, _other_generics) => {
                    if !base.of_type(self, syntax).await {
                        return false;
                    }

                    //TODO check generics, I have no clue how to with respect to subFinalizedTypes.
                    todo!()
                }
                FinalizedTypes::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound, syntax).await {
                            return false;
                        }
                    }
                    true
                }
                _ => false
            }
            FinalizedTypes::Reference(referencing) => match other {
                FinalizedTypes::Reference(other) => referencing.of_type(other, syntax).await,
                _ => false
            },
            FinalizedTypes::Generic(_, bounds) => match other {
                FinalizedTypes::Generic(_, other_bounds) => {
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
        return output;
    }

    pub async fn resolve_generic(&self, other: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>,
                                 bounds_error: ParsingError) -> Result<Option<FinalizedTypes>, ParsingError> {
        match self {
            FinalizedTypes::Generic(_name, bounds) => {
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

    pub async fn degeneric(&mut self, generics: &HashMap<String, FinalizedTypes>, syntax: &Arc<Mutex<Syntax>>,
                           none_error: ParsingError, bounds_error: ParsingError) -> Result<(), ParsingError> {
        match self {
            FinalizedTypes::Generic(name, bounds) => {
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

    #[async_recursion]
    pub async fn flatten(&mut self, generics: &mut Vec<FinalizedTypes>, syntax: &Arc<Mutex<Syntax>>) -> Result<FinalizedTypes, ParsingError> {
        for generic in &mut *generics {
            if let FinalizedTypes::GenericType(base, bounds) = generic {
                *generic = base.flatten(bounds, syntax).await?;
            }
        }
        return match self {
            FinalizedTypes::Struct(found) => {
                if generics.is_empty() {
                    return Ok(self.clone());
                }
                let name = format!("{}<{}>", found.data.name, display_parenless(generics, "_"));
                if syntax.lock().unwrap().structures.types.contains_key(&name) {
                    let data;
                    {
                        let locked = syntax.lock().unwrap();
                        data = locked.structures.types.get(&name).unwrap().clone();
                    }
                    Ok(FinalizedTypes::Struct(AsyncDataGetter::new(syntax.clone(), data).await))
                } else {
                    let mut other = StructData::clone(&found.data);
                    other.fix_id();
                    other.name = name.clone();
                    let other = Arc::new(other);
                    syntax.lock().unwrap().structures.types.insert(name, other.clone());
                    let mut data = FinalizedStruct::clone(AsyncDataGetter::new(syntax.clone(), other.clone()).await.deref());
                    data.degeneric(generics, syntax).await?;
                    let data = Arc::new(data);
                    let mut locked = syntax.lock().unwrap();
                    if let Some(wakers) = locked.structures.wakers.remove(&data.data.name) {
                        for waker in wakers {
                            waker.wake();
                        }
                    }

                    locked.structures.data.insert(other.clone(), data.clone());
                    Ok(FinalizedTypes::Struct(data))
                }
            }
            FinalizedTypes::Reference(other) => other.flatten(generics, syntax).await,
            FinalizedTypes::Generic(_, _) => panic!("Unresolved generic!"),
            FinalizedTypes::GenericType(base, effects) =>
                base.flatten(effects, syntax).await
        };
    }

    pub fn name(&self) -> String {
        return match self {
            FinalizedTypes::Struct(structs) => structs.data.name.clone(),
            FinalizedTypes::Reference(structs) => structs.name(),
            FinalizedTypes::Generic(_, _) => panic!("Generics should never be named"),
            FinalizedTypes::GenericType(_, _) => panic!("Generics should never be named")
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

impl Display for FinalizedTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinalizedTypes::Struct(structure) => write!(f, "{}", structure.data.name),
            FinalizedTypes::Reference(structure) => write!(f, "&{}", structure),
            FinalizedTypes::Generic(name, bounds) =>
                write!(f, "{}: {}", name, display(bounds, " + ")),
            FinalizedTypes::GenericType(types, generics) =>
                write!(f, "{}<{}>", types, display_parenless(generics, "_"))
        }
    }
}

impl PartialEq for FinalizedTypes {
    fn eq(&self, other: &Self) -> bool {
        return self.name() == other.name();
    }
}