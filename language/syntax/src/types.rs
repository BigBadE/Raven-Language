/// Contains all the code for interacting with types in Raven.
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use std::thread;
use chalk_ir::{BoundVar, DebruijnIndex, GenericArgData, Substitution, Ty, TyKind};
use chalk_solve::rust_ir::TraitDatum;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use async_recursion::async_recursion;
use crate::function::{display, display_parenless};
use crate::{is_modifier, Modifier, ParsingError, StructData, TopElement};
use crate::async_util::AsyncDataGetter;
use crate::chalk_interner::ChalkIr;
use crate::code::FinalizedMemberField;
use crate::r#struct::{ChalkData, FinalizedStruct};
use crate::syntax::Syntax;

/// A type is assigned to every value at compilation-time in Raven because it's statically typed.
/// For example, "test" is a Struct called str, which is an internal type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Types {
    // A basic struct
    Struct(Arc<StructData>),
    // A type with generic types. For example, List<T> is GenericType with a base struct (List) and bounds T.
    // This List<T> will be degeneric'd into a type (for example, List<String>) then solidified.
    GenericType(Box<Types>, Vec<Types>),
    // A reference to a type
    Reference(Box<Types>),
    // A generic with bounds
    Generic(String, Vec<Types>),
    // An array
    Array(Box<Types>),
}

///A type with a reference to the finalized structure instead of the data.
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
    Array(Box<FinalizedTypes>),
}

impl Types {
    /// Returns the name of the type.
    pub fn name(&self) -> String {
        return match self {
            Types::Struct(structs) => structs.name.clone(),
            Types::Reference(structs) => structs.name(),
            Types::Array(types) => format!("[{}]", types.name()),
            Types::Generic(_, _) => panic!("Generics should never be named"),
            Types::GenericType(_, _) => panic!("Generics should never be named")
        };
    }

    /// Finalized the type by waiting for the FinalizedStruct to be avalible.
    #[async_recursion]
    pub async fn finalize(&self, syntax: Arc<Mutex<Syntax>>) -> FinalizedTypes {
        return match self {
            Types::Struct(structs) =>
                {
                    FinalizedTypes::Struct(AsyncDataGetter::new(syntax, structs.clone()).await)
                },
            Types::Reference(structs) =>
                FinalizedTypes::Reference(Box::new(structs.finalize(syntax).await)),
            Types::Array(inner) => FinalizedTypes::Array(Box::new(inner.finalize(syntax).await)),
            Types::Generic(name, bounds) =>
                FinalizedTypes::Generic(name.clone(),
                                        Self::finalize_all(syntax, bounds).await),
            Types::GenericType(base, bounds) =>
                FinalizedTypes::GenericType(Box::new(base.finalize(syntax.clone()).await),
                                            Self::finalize_all(syntax, bounds).await)
        };
    }

    /// Finalizes a list of types.
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

    /// Gets the fields of the type. Useful for creating a new struct or getting data from a field of a struct.
    pub fn get_fields(&self) -> &Vec<FinalizedMemberField> {
        return match self {
            FinalizedTypes::Struct(inner) => &inner.fields,
            FinalizedTypes::Reference(inner) => inner.get_fields(),
            _ => panic!("Tried to get fields of generic!")
        };
    }

    /// Assumes the type is a trait and returns its inner Chalk Trait data.
    pub fn to_chalk_trait(&self, binders: &Vec<&String>) -> TraitDatum<ChalkIr> {
        if let FinalizedTypes::Struct(inner) = self {
            if let ChalkData::Trait(_, _, traits) = inner.data.chalk_data.as_ref().unwrap() {
                return traits.clone();
            } else {
                panic!("Expected trait, found struct!");
            }
        } else if let FinalizedTypes::GenericType(base, _) = self {
            return base.to_chalk_trait(binders);
        } else if let FinalizedTypes::Reference(inner) = self {
            return inner.to_chalk_trait(binders);
        } else {
            panic!("Expected trait, found {:?}", self);
        }
    }

    /// Converts the type into its Chalk version.
    /// Binders are Chalk's name for the generics.
    pub fn to_chalk_type(&self, binders: &Vec<&String>) -> Ty<ChalkIr> {
        return match self {
            FinalizedTypes::Struct(structure) => match &structure.data.chalk_data.as_ref().unwrap() {
                ChalkData::Struct(types, _) => types.clone(),
                ChalkData::Trait(types, _, _) => types.clone()
            },
            FinalizedTypes::Reference(inner) => inner.to_chalk_type(binders),
            FinalizedTypes::Array(inner) =>
                TyKind::Slice(inner.to_chalk_type(binders)).intern(ChalkIr),
            FinalizedTypes::Generic(name, _bounds) => {
                let index = binders.iter().position(|found| *found == name).unwrap();
                TyKind::BoundVar(BoundVar {
                    debruijn: DebruijnIndex::INNERMOST,
                    index,
                }).intern(ChalkIr)
            }
            FinalizedTypes::GenericType(inner, bounds) => {
                if let TyKind::Adt(id, _) = inner.to_chalk_type(binders).data(ChalkIr).kind {
                    let mut generic_args = Vec::new();
                    for arg in bounds {
                        generic_args.push(GenericArgData::Ty(arg.to_chalk_type(binders)).intern(ChalkIr));
                    }
                    // Returns the structure with the correct substitutions from bounds for generic types.
                    return TyKind::Adt(id,
                                       Substitution::from_iter(ChalkIr, generic_args))
                        .intern(ChalkIr);
                } else {
                    unreachable!()
                }
            }
        };
    }

    /// Assumes the type is a struct and returns that struct.
    pub fn inner_struct(&self) -> &Arc<FinalizedStruct> {
        return match self {
            FinalizedTypes::Struct(structure) => structure,
            FinalizedTypes::Reference(inner) => inner.inner_struct(),
            FinalizedTypes::GenericType(inner, _) => inner.inner_struct(),
            _ => panic!("Tried to get inner struct of invalid type! {:?}", self)
        };
    }

    /// Checks if the type is of the other type, following Raven's type rules.
    /// May block until all implementations are finished parsing, must not be called from
    /// implementation parsing to prevent deadlocking.
    pub fn of_type(&self, other: &FinalizedTypes, syntax: Option<&Arc<Mutex<Syntax>>>) -> bool {
        return match self {
            FinalizedTypes::Struct(found) => match other {
                FinalizedTypes::Struct(other_struct) => {
                    if found == other_struct {
                        true
                    } else if is_modifier(other.inner_struct().data.modifiers, Modifier::Trait) {
                        if syntax.is_none() {
                            return false;
                        }
                        let syntax = syntax.unwrap();
                        // Only check for implementations if being compared against a trait.
                        // Wait for the implementation to finish.
                        while !syntax.lock().unwrap().finished_impls() {
                            if syntax.lock().unwrap().solve(self, &other) {
                                return true;
                            }
                            thread::yield_now();
                        }
                        // Now all impls are parsed so solve is correct.
                        return syntax.lock().unwrap().solve(self, &other);
                    } else {
                        false
                    }
                }
                FinalizedTypes::Generic(_, bounds) => {
                    // If any bounds fail, the type isn't of the generic.
                    for bound in bounds {
                        if !other.of_type(bound, syntax) {
                            return false;
                        }
                    }
                    true
                }
                // For structures vs generic types, just check the base.
                FinalizedTypes::GenericType(base, _) => self.of_type(base, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(inner) => self.of_type(inner, syntax),
                FinalizedTypes::Array(_) => false
            },
            FinalizedTypes::Array(inner) => match other {
                // Check the inner type.
                FinalizedTypes::Array(other) => inner.of_type(other, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(other) => self.of_type(other, syntax),
                // Only arrays can equal arrays
                _ => false
            },
            FinalizedTypes::GenericType(base, generics) => match other {
                FinalizedTypes::GenericType(other_base, other_generics) => {
                    if generics.len() != other_generics.len() || !base.of_type(other_base, syntax) {
                        return false;
                    }

                    for i in 0..generics.len() {
                        if !generics[i].of_type(&other_generics[i], syntax) {
                            return false;
                        }
                    }
                    return true;
                }
                FinalizedTypes::Generic(_, bounds) => {
                    // Check each bound, if any are violated it's not of the generic type.
                    for bound in bounds {
                        if !self.of_type(bound, syntax) {
                            return false;
                        }
                    }
                    true
                }
                // Against structures just check the base.
                FinalizedTypes::Struct(_) => base.of_type(other, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(inner) => self.of_type(inner, syntax),
                FinalizedTypes::Array(_) => false
            }
            // References are ignored for type checking.
            FinalizedTypes::Reference(referencing) => referencing.of_type(other, syntax),
            FinalizedTypes::Generic(_, bounds) => match other {
                FinalizedTypes::Generic(_, other_bounds) => {
                    // For two generics to be the same, each bound must match at least one other bound.
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
                // Flip it, because every other type handles generics already, no need to repeat the code.
                _ => other.of_type(self, syntax)
            }
        };
    }

    /// Compares one type against another type to try and solidify any generic types.
    /// Errors if the other type isn't of this type.
    #[async_recursion]
    pub async fn resolve_generic(&self, other: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>,
                                 generics: &mut HashMap<String, FinalizedTypes>, bounds_error: ParsingError)
                                 -> Result<(), ParsingError> {
        match self {
            FinalizedTypes::Generic(name, bounds) => {
                // Check for bound errors.
                for bound in bounds {
                    if !other.of_type(bound, Some(syntax)) {
                        return Err(bounds_error);
                    }
                }
                generics.insert(name.clone(), other.clone());
            }
            FinalizedTypes::GenericType(base, bounds) => {
                let mut other = other;
                // Ignore references.
                while let FinalizedTypes::Reference(inner) = other {
                    other = inner;
                }

                if let FinalizedTypes::GenericType(other_base, other_bounds) = other {
                    if other_bounds.len() != bounds.len() {
                        return Err(bounds_error);
                    }
                    base.resolve_generic(other_base, syntax, generics, bounds_error.clone()).await?;

                    for i in 0..bounds.len() {
                        bounds[i].resolve_generic(&other_bounds[i], syntax, generics, bounds_error.clone()).await?;
                    }
                }
            }
            // Ignore references.
            FinalizedTypes::Reference(inner) => {
                return inner.resolve_generic(other, syntax, generics, bounds_error).await;
            }
            FinalizedTypes::Array(inner) => {
                let mut other = other;
                // Ignore references.
                while let FinalizedTypes::Reference(inner) = other {
                    other = inner;
                }
                // Check on the inner type.
                if let FinalizedTypes::Array(other) = other {
                    return inner.resolve_generic(other, syntax, generics, bounds_error).await;
                }
                return Err(bounds_error);
            }
            _ => {}
        }
        return Ok(());
    }

    /// Degenerics the type by replacing all generics with their solidified value.
    #[async_recursion]
    pub async fn degeneric(&mut self, generics: &HashMap<String, FinalizedTypes>, syntax: &Arc<Mutex<Syntax>>,
                           mut none_error: ParsingError, bounds_error: ParsingError) -> Result<(), ParsingError> {
        return match self {
            FinalizedTypes::Generic(name, bounds) => {
                if let Some(found) = generics.get(name) {
                    // This should never trip, but it's a sanity check.
                    for bound in bounds {
                        if !found.of_type(bound, Some(syntax)) {
                            return Err(bounds_error);
                        }
                    }
                    *self = found.clone();
                    Ok(())
                } else {
                    none_error.message = format!("{}: {} and {:?}", none_error.message, self, generics.keys().collect::<Vec<_>>());
                    Err(none_error)
                }
            }
            FinalizedTypes::GenericType(base, bounds) => {
                base.degeneric(generics, syntax, none_error.clone(), bounds_error.clone()).await?;
                let mut found = Vec::new();
                for bound in bounds {
                    bound.degeneric(generics, syntax, none_error.clone(), bounds_error.clone()).await?;
                    found.push(bound.clone());
                }
                *self = base.flatten(&mut found, syntax).await?;
                Ok(())
            }
            FinalizedTypes::Reference(inner) => {
                inner.degeneric(generics, syntax, none_error, bounds_error).await
            }
            FinalizedTypes::Array(inner) => {
                inner.degeneric(generics, syntax, none_error, bounds_error).await
            }
            _ => Ok(())
        };
    }

    /// Flattens GenericTypes into a Structure, degenericing them.
    #[async_recursion]
    pub async fn flatten(&self, generics: &Vec<FinalizedTypes>, syntax: &Arc<Mutex<Syntax>>) -> Result<FinalizedTypes, ParsingError> {
        return match self {
            FinalizedTypes::Struct(found) => {
                if generics.is_empty() {
                    // If there are no bounds, we're good.
                    return Ok(self.clone());
                }
                let name = format!("{}<{}>", found.data.name, display_parenless(generics, ", "));
                // If this type has already been flattened with these args, return that.
                if syntax.lock().unwrap().structures.types.contains_key(&name) {
                    let data;
                    {
                        let locked = syntax.lock().unwrap();
                        data = locked.structures.types.get(&name).unwrap().clone();
                    }
                    Ok(FinalizedTypes::Struct(AsyncDataGetter::new(syntax.clone(), data).await))
                } else {
                    // Clone the type and add the new type to the structures.
                    let mut other = StructData::clone(&found.data);
                    other.name = name.clone();
                    let arc_other;
                    {
                        let mut locked = syntax.lock().unwrap();
                        other.set_id(locked.structures.sorted.len() as u64);
                        arc_other = Arc::new(other);
                        locked.structures.types.insert(name, arc_other.clone());
                        locked.structures.sorted.push(arc_other.clone());
                    }
                    // Get the FinalizedStruct and degeneric it.
                    let mut data = FinalizedStruct::clone(AsyncDataGetter::new(syntax.clone(), arc_other.clone()).await.deref());
                    data.degeneric(generics, syntax).await?;
                    let data = Arc::new(data);
                    // Add the flattened type to the
                    let mut locked = syntax.lock().unwrap();
                    if let Some(wakers) = locked.structures.wakers.remove(&data.data.name) {
                        for waker in wakers {
                            waker.wake();
                        }
                    }

                    locked.structures.data.insert(arc_other.clone(), data.clone());
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

    pub fn name_safe(&self) -> Option<String> {
        return match self {
            FinalizedTypes::Struct(structs) => Some(structs.data.name.clone()),
            FinalizedTypes::Reference(structs) => structs.name_safe(),
            FinalizedTypes::Array(inner) => inner.name_safe().map(|inner| format!("[{}]", inner)),
            FinalizedTypes::Generic(_, _) => None,
            FinalizedTypes::GenericType(_, _) => None
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
                write!(f, "{}<{}>", types, display_parenless(generics, ", "))
        }
    }
}

impl Display for FinalizedTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinalizedTypes::Struct(structure) => write!(f, "{}", structure.data.name),
            FinalizedTypes::Reference(structure) => write!(f, "&{}", structure),
            FinalizedTypes::Array(inner) => write!(f, "[{}]", inner),
            FinalizedTypes::Generic(name, bounds) =>
                write!(f, "{}: {}", name, display(bounds, " + ")),
            FinalizedTypes::GenericType(types, generics) =>
                write!(f, "{}<{}>", types, display_parenless(generics, "_"))
        }
    }
}

impl PartialEq for FinalizedTypes {
    fn eq(&self, other: &Self) -> bool {
        return self.name_safe().map(|inner| other.name_safe()
            .map(|other| inner == other).unwrap_or(false)).unwrap_or(false);
    }
}