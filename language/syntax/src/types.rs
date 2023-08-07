use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use chalk_integration::interner::ChalkIr;
use chalk_ir::{BoundVar, DebruijnIndex, GenericArgData, Substitution, Ty, TyKind};
use chalk_solve::rust_ir::TraitDatum;
use no_deadlocks::Mutex;
use async_recursion::async_recursion;
use crate::function::{display, display_parenless};
use crate::{is_modifier, Modifier, ParsingError, StructData};
use crate::async_util::AsyncDataGetter;
use crate::code::FinalizedMemberField;
use crate::r#struct::{ChalkData, FinalizedStruct};
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
    //An array
    Array(Box<Types>)
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
    //An array
    Array(Box<FinalizedTypes>)
}

impl Types {
    pub fn name(&self) -> String {
        return match self {
            Types::Struct(structs) => structs.name.clone(),
            Types::Reference(structs) => structs.name(),
            Types::Array(types) => format!("[{}]", types.name()),
            Types::Generic(_, _) => panic!("Generics should never be named"),
            Types::GenericType(_, _) => panic!("Generics should never be named")
        };
    }

    #[async_recursion]
    pub async fn finalize(&self, syntax: Arc<Mutex<Syntax>>) -> FinalizedTypes {
        return match self {
            Types::Struct(structs) => FinalizedTypes::Struct(AsyncDataGetter::new(syntax, structs.clone()).await),
            Types::Reference(structs) => FinalizedTypes::Reference(Box::new(structs.finalize(syntax).await)),
            Types::Array(inner) => FinalizedTypes::Array(Box::new(inner.finalize(syntax).await)),
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

    pub fn to_trait(&self, binders: &Vec<&String>) -> TraitDatum<ChalkIr> {
        if let FinalizedTypes::Struct(inner) = self {
            if let ChalkData::Trait(traits) = &inner.data.chalk_data {
                return traits.clone();
            } else {
                panic!("Expected trait, found struct!");
            }
        } else if let FinalizedTypes::GenericType(base, _) = self {
            return base.to_trait(binders);
        } else if let FinalizedTypes::Reference(inner) = self {
            return inner.to_trait(binders);
        } else {
            panic!("Expected trait, found {:?}", self);
        }
    }

    pub fn to_chalk_type(&self, binders: &Vec<&String>) -> Ty<ChalkIr> {
        return match self {
            FinalizedTypes::Struct(structure) =>
                if let ChalkData::Struct(types, _) = &structure.data.chalk_data {
                    return types.clone();
                } else {
                    panic!("Tried to get chalk type of struct!")
                },
            FinalizedTypes::Reference(inner) => inner.to_chalk_type(binders),
            FinalizedTypes::Array(inner) => TyKind::Slice(inner.to_chalk_type(binders)).intern(ChalkIr),
            FinalizedTypes::Generic(name, _bounds) => {
                let index = binders.iter().position(|found| *found == name).unwrap();
                TyKind::BoundVar(BoundVar {
                    debruijn: DebruijnIndex::INNERMOST,
                    index,
                }).intern(ChalkIr)
            },
            FinalizedTypes::GenericType(inner, bounds) => {
                if let TyKind::Adt(id, _) = inner.to_chalk_type(binders).data(ChalkIr).kind {
                    let mut generic_args = Vec::new();
                    for arg in bounds {
                        generic_args.push(GenericArgData::Ty(arg.to_chalk_type(binders)).intern(ChalkIr));
                    }
                    return TyKind::Adt(id,
                                       Substitution::from_iter(ChalkIr, generic_args))
                        .intern(ChalkIr);
                } else {
                    panic!()
                }
            }
        }
    }

    pub fn inner_struct(&self) -> &Arc<FinalizedStruct> {
        return match self {
            FinalizedTypes::Struct(structure) => structure,
            FinalizedTypes::Reference(inner) => inner.inner_struct(),
            FinalizedTypes::GenericType(inner, _) => inner.inner_struct(),
            _ => panic!("Tried to get inner struct of invalid type! {:?}", self)
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

    pub fn of_type(&self, other: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>) -> bool {
        let output = match self {
            FinalizedTypes::Struct(found) => match other {
                FinalizedTypes::Struct(other_struct) => {
                    if found == other_struct {
                        true
                    } else if is_modifier(other.inner_struct().data.modifiers, Modifier::Trait) {
                        println!("Comparing {} to {}", self, other);
                        //Only check for implementations if being compared against a trait.
                        syntax.lock().unwrap().solve(&found.data, &other_struct.data)
                    } else {
                        false
                    }
                },
                FinalizedTypes::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound, syntax) {
                            return false;
                        }
                    }
                    true
                }
                FinalizedTypes::GenericType(base, _) => self.of_type(base, syntax),
                FinalizedTypes::Reference(inner) => self.of_type(inner, syntax),
                FinalizedTypes::Array(_) => false
            },
            FinalizedTypes::Array(inner) => match other {
                FinalizedTypes::Array(other) => inner.of_type(other, syntax),
                FinalizedTypes::Reference(other) => self.of_type(other, syntax),
                _ => false
            },
            FinalizedTypes::GenericType(base, _generics) => match other {
                FinalizedTypes::GenericType(_other_base, _other_generics) => {
                    if !base.of_type(self, syntax) {
                        return false;
                    }

                    //TODO check generics, I have no clue how to with respect to subFinalizedTypes.
                    todo!()
                }
                FinalizedTypes::Generic(_, bounds) => {
                    for bound in bounds {
                        if !self.of_type(bound, syntax) {
                            return false;
                        }
                    }
                    true
                },
                FinalizedTypes::Struct(_) => {
                    base.of_type(other, syntax)
                },
                FinalizedTypes::Reference(inner) => self.of_type(inner, syntax),
                FinalizedTypes::Array(_) => false
            }
            FinalizedTypes::Reference(referencing) => referencing.of_type(other, syntax),
            FinalizedTypes::Generic(_, bounds) => match other {
                FinalizedTypes::Generic(_, other_bounds) => {
                    'outer: for bound in bounds {
                        for other_bound in other_bounds {
                            if other_bound.of_type(bound, syntax) {
                                continue 'outer;
                            }
                        }
                        return false;
                    }
                    true
                }
                _ => other.of_type(self, syntax)
            }
        };
        return output;
    }

    pub async fn resolve_generic(&self, other: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>,
                                 bounds_error: ParsingError) -> Result<Option<FinalizedTypes>, ParsingError> {
        match self {
            FinalizedTypes::Generic(_name, bounds) => {
                for bound in bounds {
                    if !other.of_type(bound, syntax) {
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
                        if !found.of_type(bound, syntax) {
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
                    syntax.lock().unwrap().structures.sorted.push(other.clone());
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
            FinalizedTypes::Array(inner) => inner.flatten(generics, syntax).await,
            FinalizedTypes::Generic(_, _) => panic!("Unresolved generic!"),
            FinalizedTypes::GenericType(base, effects) =>
                base.flatten(effects, syntax).await
        };
    }

    pub fn name(&self) -> String {
        return match self {
            FinalizedTypes::Struct(structs) => structs.data.name.clone(),
            FinalizedTypes::Reference(structs) => structs.name(),
            FinalizedTypes::Array(inner) => format!("[{}]", inner.name()),
            FinalizedTypes::Generic(name, _) => panic!("Generics should never be named, tried to get {}", name),
            FinalizedTypes::GenericType(_, _) => panic!("Generics should never be named")
        };
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::Struct(structure) => write!(f, "{}", structure.name),
            Types::Reference(structure) => write!(f, "&{}", structure),
            Types::Array(inner) => write!(f, "[{}]", inner),
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
            FinalizedTypes::Array(inner) => write!(f, "{}[]", inner),
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