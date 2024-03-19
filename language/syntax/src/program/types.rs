use parking_lot::Mutex;
/// Contains all the code for interacting with types in Raven.
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chalk_ir::{BoundVar, DebruijnIndex, GenericArgData, Substitution, Ty, TyKind};
use chalk_solve::rust_ir::TraitDatum;

use async_recursion::async_recursion;
use data::tokens::Span;

use crate::async_util::AsyncDataGetter;
use crate::chalk_interner::ChalkIr;
use crate::errors::{ErrorSource, ParsingMessage};
use crate::program::code::FinalizedMemberField;
use crate::program::function::{display, display_parenless, FunctionData};
use crate::program::r#struct::{ChalkData, FinalizedStruct};
use crate::program::syntax::Syntax;
use crate::top_element_manager::{ImplWaiter, TypeImplementsTypeWaiter};
use crate::{is_modifier, Modifier, ParsingError, StructData};

/// A type is assigned to every value at compilation-time in Raven because it's statically typed.
/// For example, "test" is a Struct called str, which is an internal type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Types {
    /// A basic struct
    Struct(Arc<StructData>),
    /// A type with generic types. For example, List<T> is GenericType with a base struct (List) and bounds T.
    /// This List<T> will be degeneric'd into a type (for example, List<String>) then solidified.
    GenericType(Box<Types>, Vec<Types>),
    /// A reference to a type
    Reference(Box<Types>),
    /// A generic with bounds
    Generic(String, Vec<Types>),
    /// An array
    Array(Box<Types>),
}

///A type with a reference to the finalized program instead of the data.
#[derive(Clone, Debug, Eq, Hash)]
pub enum FinalizedTypes {
    /// A basic struct
    Struct(Arc<FinalizedStruct>),
    /// A type with generic types
    GenericType(Box<FinalizedTypes>, Vec<FinalizedTypes>),
    /// A reference to a type
    Reference(Box<FinalizedTypes>),
    /// A generic with bounds
    Generic(String, Vec<FinalizedTypes>),
    /// An array
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
            Types::GenericType(_, _) => panic!("Generics should never be named"),
        };
    }

    /// Finalized the type by waiting for the FinalizedStruct to be avalible.
    #[async_recursion(Sync)]
    pub async fn finalize(&self, syntax: Arc<Mutex<Syntax>>) -> FinalizedTypes {
        return match self {
            Types::Struct(structs) => FinalizedTypes::Struct(AsyncDataGetter::new(syntax, structs.clone()).await),
            Types::Reference(structs) => FinalizedTypes::Reference(Box::new(structs.finalize(syntax).await)),
            Types::Array(inner) => FinalizedTypes::Array(Box::new(inner.finalize(syntax).await)),
            Types::Generic(name, bounds) => FinalizedTypes::Generic(name.clone(), Self::finalize_all(syntax, bounds).await),
            Types::GenericType(base, bounds) => FinalizedTypes::GenericType(
                Box::new(base.finalize(syntax.clone()).await),
                Self::finalize_all(syntax, bounds).await,
            ),
        };
    }

    /// Finalizes a list of types.
    async fn finalize_all(syntax: Arc<Mutex<Syntax>>, types: &Vec<Types>) -> Vec<FinalizedTypes> {
        let mut output = Vec::default();
        for found in types {
            output.push(found.finalize(syntax.clone()).await);
        }
        return output;
    }
}

impl FinalizedTypes {
    /// The ID of the type
    pub fn id(&self) -> u64 {
        return match self {
            FinalizedTypes::Struct(structure) => structure.data.id,
            FinalizedTypes::Reference(inner) => inner.id(),
            _ => panic!("Tried to ID generic!"),
        };
    }

    /// Gets the fields of the type. Useful for creating a new struct or getting data from a field of a struct.
    pub fn get_fields(&self) -> &Vec<FinalizedMemberField> {
        return match self {
            FinalizedTypes::Struct(inner) => &inner.fields,
            FinalizedTypes::Reference(inner) => inner.get_fields(),
            FinalizedTypes::GenericType(base, _) => base.get_fields(),
            _ => panic!("Tried to get fields of generic!"),
        };
    }

    /// Finds all methods with the name from the type
    pub fn find_method(&self, name: &String) -> Option<Vec<(FinalizedTypes, Arc<FunctionData>)>> {
        return match self {
            FinalizedTypes::Struct(inner) => inner
                .data
                .functions
                .iter()
                .find(|inner| inner.name.ends_with(name))
                .map(|inner| vec![(self.clone(), inner.clone())]),
            FinalizedTypes::Reference(inner) => inner.find_method(name),
            FinalizedTypes::GenericType(base, _) => base.find_method(name),
            FinalizedTypes::Generic(_, bounds) => {
                let mut output = vec![];
                for bound in bounds {
                    if let Some(found) = bound.find_method(name) {
                        for temp in found {
                            output.push(temp);
                        }
                    }
                }
                if output.is_empty() {
                    None
                } else {
                    Some(output)
                }
            }
            FinalizedTypes::Array(_) => None,
        };
    }

    /// Assumes the type is a trait and returns its inner Chalk Trait data.
    pub fn to_chalk_trait(&self, binders: &Vec<&String>) -> TraitDatum<ChalkIr> {
        if let FinalizedTypes::Struct(inner) = self {
            if let ChalkData::Trait(_, _, traits) = &inner.data.chalk_data {
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
            FinalizedTypes::Struct(structure) => {
                match &structure.data.chalk_data {
                    ChalkData::Struct(types, _) => types.clone(), // skipcq: RS-W1110 types isn't Copy
                    ChalkData::Trait(types, _, _) => types.clone(), // skipcq: RS-W1110 types isn't Copy
                }
            }
            FinalizedTypes::Reference(inner) => inner.to_chalk_type(binders),
            FinalizedTypes::Array(inner) => TyKind::Slice(inner.to_chalk_type(binders)).intern(ChalkIr),
            FinalizedTypes::Generic(name, _bounds) => {
                let index = binders.iter().position(|found| *found == name).unwrap();
                TyKind::BoundVar(BoundVar { debruijn: DebruijnIndex::INNERMOST, index }).intern(ChalkIr)
            }
            FinalizedTypes::GenericType(inner, bounds) => {
                if let TyKind::Adt(id, _) = inner.to_chalk_type(binders).data(ChalkIr).kind {
                    let mut generic_args = Vec::default();
                    for arg in bounds {
                        generic_args.push(GenericArgData::Ty(arg.to_chalk_type(binders)).intern(ChalkIr));
                    }
                    // Returns the program with the correct substitutions from bounds for generic types.
                    return TyKind::Adt(id, Substitution::from_iter(ChalkIr, generic_args)).intern(ChalkIr);
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
            _ => panic!("Tried to get inner struct of invalid type! {:?}", self),
        };
    }

    /// Assumes the type is a struct and returns that struct.
    pub fn inner_struct_safe(&self) -> Option<&Arc<FinalizedStruct>> {
        return match self {
            FinalizedTypes::Struct(structure) => Some(structure),
            FinalizedTypes::Reference(inner) => inner.inner_struct_safe(),
            FinalizedTypes::GenericType(inner, _) => inner.inner_struct_safe(),
            _ => None,
        };
    }

    /// Gets the inner generic type from a type
    pub fn inner_generic_type(&self) -> Option<(&Box<FinalizedTypes>, &Vec<FinalizedTypes>)> {
        return match self {
            FinalizedTypes::GenericType(inner, bounds) => Some((inner, bounds)),
            FinalizedTypes::Reference(inner) => inner.inner_generic_type(),
            _ => None,
        };
    }

    /// Checks if the type is of the other type, following Raven's type rules.
    /// May block until all implementations are finished parsing, must not be called from
    /// implementation parsing to prevent deadlocking.
    pub async fn of_type(&self, other: &FinalizedTypes, syntax: Arc<Mutex<Syntax>>) -> bool {
        let (result, future) = self.of_type_sync(other, Some(syntax));
        return if result {
            true
        } else if let Some(found) = future {
            found.await
        } else {
            false
        };
    }

    /// This method doesn't block, instead it returns a future which can be waited on if a blocking
    /// result is wanted. This waiter is only there is syntax is Some.
    // skipcq: RS-R1000 Match statements have complexity calculated incorrectly
    pub fn of_type_sync(
        &self,
        other: &FinalizedTypes,
        syntax: Option<Arc<Mutex<Syntax>>>,
    ) -> (bool, Option<Pin<Box<dyn Future<Output = bool> + Send + Sync>>>) {
        return match self {
            FinalizedTypes::Struct(found) => match other {
                FinalizedTypes::Struct(other_struct) => {
                    if found == other_struct {
                        (true, None)
                    } else if is_modifier(other.inner_struct().data.modifiers, Modifier::Trait) {
                        if syntax.is_none() {
                            return (false, None);
                        }
                        return (
                            false,
                            Some(Box::pin(TypeImplementsTypeWaiter {
                                syntax: syntax.unwrap().clone(),
                                current: self.clone(),
                                other: other.clone(),
                            })),
                        );
                    } else {
                        (false, None)
                    }
                }
                FinalizedTypes::Generic(_, bounds) => {
                    // If any bounds fail, the type isn't of the generic.
                    let mut fails = Vec::default();
                    for bound in bounds {
                        let (result, future) = self.of_type_sync(bound, syntax.clone());
                        if !result {
                            if let Some(found) = future {
                                fails.push(found);
                            } else {
                                return (false, None);
                            }
                        }
                    }
                    if !fails.is_empty() {
                        return (false, Some(Box::pin(Self::join(fails))));
                    }
                    (true, None)
                }
                // For structures vs generic types, just check the base.
                FinalizedTypes::GenericType(base, _) => self.of_type_sync(base, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(inner) => self.of_type_sync(inner, syntax),
                FinalizedTypes::Array(_) => (false, None),
            },
            FinalizedTypes::Array(inner) => match other {
                // Check the inner type.
                FinalizedTypes::Array(other) => inner.of_type_sync(other, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(other) => self.of_type_sync(other, syntax),
                // Only arrays can equal arrays
                _ => (false, None),
            },
            FinalizedTypes::GenericType(base, generics) => match other {
                FinalizedTypes::GenericType(other_base, other_generics) => {
                    if base != other_base {
                        return (false, Some(Box::pin(Self::get_has_impl(syntax, self.clone(), other.clone()))));
                    }

                    let mut fails = Vec::default();
                    if generics.len() != other_generics.len() {
                        return (false, None);
                    }

                    for i in 0..generics.len() {
                        let (result, future) = generics[i].of_type_sync(&other_generics[i], syntax.clone());
                        if !result {
                            if let Some(found) = future {
                                fails.push(found);
                            } else {
                                return (false, None);
                            }
                        }
                    }
                    if !fails.is_empty() {
                        return (false, Some(Box::pin(Self::join(fails))));
                    }
                    (true, None)
                }
                FinalizedTypes::Generic(_, bounds) => {
                    let mut fails = Vec::default();
                    // Check each bound, if any are violated it's not of the generic type.
                    for bound in bounds {
                        let (result, future) = self.of_type_sync(bound, syntax.clone());
                        if !result {
                            if let Some(found) = future {
                                fails.push(found);
                            } else {
                                return (false, None);
                            }
                        }
                    }
                    if !fails.is_empty() {
                        return (false, Some(Box::pin(Self::join(fails))));
                    }
                    (true, None)
                }
                // Against structures just check the base.
                FinalizedTypes::Struct(_) => base.of_type_sync(other, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(inner) => self.of_type_sync(inner, syntax),
                FinalizedTypes::Array(_) => (false, None),
            },
            // References are ignored for type checking.
            FinalizedTypes::Reference(referencing) => referencing.of_type_sync(other, syntax),
            FinalizedTypes::Generic(_, bounds) => match other {
                FinalizedTypes::Generic(_, other_bounds) => {
                    let mut outer_fails: Vec<Pin<Box<dyn Future<Output = bool> + Send + Sync>>> = Vec::default();
                    // For two generics to be the same, each bound must match at least one other bound.
                    'outer: for bound in bounds {
                        let mut fails = Vec::default();
                        for other_bound in other_bounds {
                            let (result, failure) = other_bound.of_type_sync(bound, syntax.clone());
                            if result {
                                continue 'outer;
                            } else if let Some(found) = failure {
                                fails.push(found);
                            }
                        }
                        if !fails.is_empty() {
                            outer_fails.push(Box::pin(Self::join(fails)));
                        } else {
                            return (false, None);
                        }
                    }
                    if !outer_fails.is_empty() {
                        return (false, Some(Box::pin(Self::join(outer_fails))));
                    }

                    (true, None)
                }
                FinalizedTypes::Reference(inner) => self.of_type_sync(inner, syntax),
                FinalizedTypes::Struct(_) | FinalizedTypes::GenericType(_, _) | FinalizedTypes::Array(_) => {
                    if bounds.is_empty() {
                        return (true, None);
                    }
                    let mut fails = Vec::default();
                    for bound in bounds {
                        let (result, failure) = bound.of_type_sync(other, syntax.clone());
                        if result {
                            return (true, None);
                        } else if let Some(found) = failure {
                            fails.push(found);
                        }
                    }
                    return if !fails.is_empty() { (false, Some(Box::pin(Self::join(fails)))) } else { (false, None) };
                }
            },
        };
    }

    pub async fn get_has_impl(syntax: Option<Arc<Mutex<Syntax>>>, base: FinalizedTypes, trait_type: FinalizedTypes) -> bool {
        return ImplWaiter {
            syntax: syntax.unwrap(),
            base_type: base,
            trait_type,
            error: Span::default().make_error(ParsingMessage::ShouldntSee("get_has_impl")),
        }
        .await
        .is_ok();
    }

    /// Joins a vec of futures, waiting for all to finish and returning true if they all returned true
    pub async fn join(joining: Vec<Pin<Box<dyn Future<Output = bool> + Send + Sync>>>) -> bool {
        for temp in joining {
            if !temp.await {
                return false;
            }
        }
        return true;
    }

    /// Compares one type against another type to try and solidify any generic types.
    /// Errors if the other type isn't of this type.
    #[async_recursion(Sync)]
    pub async fn resolve_generic(
        &self,
        other: &FinalizedTypes,
        syntax: &Arc<Mutex<Syntax>>,
        generics: &mut HashMap<String, FinalizedTypes>,
        bounds_error: Span,
    ) -> Result<(), ParsingError> {
        if !self.of_type_sync(other, None).0 && self.inner_struct_safe().is_some() {
            loop {
                let waiter = ImplWaiter {
                    syntax: syntax.clone(),
                    base_type: other.clone(),
                    trait_type: self.clone(),
                    error: bounds_error.make_error(ParsingMessage::ShouldntSee("Resolve generic")),
                };
                match waiter.await {
                    Ok(implementors) => {
                        if implementors.len() > 1 {
                            panic!("Ambiguous impl! Raven can't handle this yet!");
                        }
                        self.resolve_generic(&implementors[0].0.target, syntax, generics, bounds_error.clone()).await?;
                        implementors[0].0.base.resolve_generic(&other, syntax, generics, bounds_error.clone()).await?;
                        return Ok(());
                    }
                    Err(_) => break,
                }
            }
        }

        match self {
            FinalizedTypes::Generic(name, bounds) => {
                // Check for bound errors.
                for bound in bounds {
                    if !other.of_type(bound, syntax.clone()).await {
                        return Err(bounds_error.make_error(ParsingMessage::MismatchedTypes(other.clone(), bound.clone())));
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
                        return Err(bounds_error.make_error(ParsingMessage::IncorrectBoundsLength()));
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

                return Err(bounds_error.make_error(ParsingMessage::MismatchedTypes(other.clone(), *inner.clone())));
            }
            _ => {}
        }
        return Ok(());
    }

    /// The name of the function
    pub fn name(&self) -> String {
        return match self {
            FinalizedTypes::Struct(structs) => structs.data.name.clone(),
            FinalizedTypes::Reference(structs) => structs.name(),
            FinalizedTypes::Array(inner) => format!("[{}]", inner.name()),
            FinalizedTypes::Generic(name, _) => {
                panic!("Generics should never be named, tried to get {}", name)
            }
            FinalizedTypes::GenericType(_, _) => panic!("Generics should never be named"),
        };
    }

    /// The name of the function, not erroring if the name can't be gotten
    /// Can be used to check if a type is generic or not
    pub fn name_safe(&self) -> Option<String> {
        return match self {
            FinalizedTypes::Struct(structs) => Some(structs.data.name.clone()),
            FinalizedTypes::Reference(structs) => structs.name_safe(),
            FinalizedTypes::Array(inner) => inner.name_safe().map(|inner| format!("[{}]", inner)),
            FinalizedTypes::Generic(_, _) => None,
            FinalizedTypes::GenericType(_, _) => None,
        };
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::Struct(structure) => write!(f, "{}", structure.name),
            Types::Reference(structure) => write!(f, "{}", structure),
            Types::Array(inner) => write!(f, "[{}]", inner),
            Types::Generic(name, bounds) => write!(f, "{}: {}", name, display(bounds, " + ")),
            Types::GenericType(types, generics) => {
                write!(f, "{}<{}>", types, display_parenless(generics, ", "))
            }
        }
    }
}

impl Display for FinalizedTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FinalizedTypes::Struct(structure) => write!(f, "{}", structure.data.name),
            FinalizedTypes::Reference(structure) => write!(f, "{}", structure),
            FinalizedTypes::Array(inner) => write!(f, "[{}]", inner),
            FinalizedTypes::Generic(name, bounds) => {
                write!(f, "{}: {}", name, display(bounds, " + "))
            }
            FinalizedTypes::GenericType(types, generics) => {
                write!(f, "{}<{}>", types, display_parenless(generics, "_"))
            }
        }
    }
}

impl PartialEq for FinalizedTypes {
    fn eq(&self, other: &Self) -> bool {
        return self.name_safe().map_or(false, |inner| {
            other
                .name_safe()
                .map_or(false, |other| inner.splitn(2, '$').next().unwrap() == other.splitn(2, '$').next().unwrap())
        });
    }
}
