/// Contains all the code for interacting with types in Raven.
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use chalk_ir::{BoundVar, DebruijnIndex, GenericArgData, Substitution, Ty, TyKind};
use chalk_solve::rust_ir::TraitDatum;

use async_recursion::async_recursion;

use crate::async_util::{AsyncDataGetter, NameResolver};
use crate::chalk_interner::ChalkIr;
use crate::code::FinalizedMemberField;
use crate::function::{display, display_parenless, FunctionData};
use crate::r#struct::{ChalkData, FinalizedStruct};
use crate::syntax::Syntax;
use crate::top_element_manager::TypeWaiter;
use crate::{is_modifier, Modifier, ParsingError, StructData, TopElement};

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
    //A basic struct and the original type (if it was flattened)
    Struct(Arc<FinalizedStruct>, Option<Box<FinalizedTypes>>),
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
            Types::GenericType(_, _) => panic!("Generics should never be named"),
        };
    }

    /// Finalized the type by waiting for the FinalizedStruct to be avalible.
    #[async_recursion]
    pub async fn finalize(&self, syntax: Arc<Mutex<Syntax>>) -> FinalizedTypes {
        return match self {
            Types::Struct(structs) => {
                FinalizedTypes::Struct(AsyncDataGetter::new(syntax, structs.clone()).await, None)
            }
            Types::Reference(structs) => {
                FinalizedTypes::Reference(Box::new(structs.finalize(syntax).await))
            }
            Types::Array(inner) => FinalizedTypes::Array(Box::new(inner.finalize(syntax).await)),
            Types::Generic(name, bounds) => {
                FinalizedTypes::Generic(name.clone(), Self::finalize_all(syntax, bounds).await)
            }
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
    pub fn id(&self) -> u64 {
        return match self {
            FinalizedTypes::Struct(structure, _) => structure.data.id,
            FinalizedTypes::Reference(inner) => inner.id(),
            _ => panic!("Tried to ID generic!"),
        };
    }

    #[async_recursion]
    pub async fn fix_generics(
        &mut self,
        resolver: &dyn NameResolver,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> Result<(), ParsingError> {
        match self {
            FinalizedTypes::Struct(_, inner) => {
                if let Some(found) = inner {
                    found.fix_generics(resolver, syntax).await?;
                }
            }
            FinalizedTypes::Reference(inner) => inner.fix_generics(resolver, syntax).await?,
            FinalizedTypes::Array(inner) => inner.fix_generics(resolver, syntax).await?,
            FinalizedTypes::Generic(name, bounds) => {
                let found = &resolver.generics()[name];
                let mut temp = vec![];
                for bound in found {
                    temp.push(
                        Syntax::parse_type(
                            syntax.clone(),
                            ParsingError::empty(),
                            resolver.boxed_clone(),
                            bound.clone(),
                            vec![],
                        )
                        .await?
                        .finalize(syntax.clone())
                        .await,
                    )
                }
                *bounds = temp;
            }
            FinalizedTypes::GenericType(base, bounds) => {
                base.fix_generics(resolver, syntax).await?;
                for bound in bounds {
                    bound.fix_generics(resolver, syntax).await?;
                }
            }
        }
        return Ok(());
    }

    /// Gets the fields of the type. Useful for creating a new struct or getting data from a field of a struct.
    pub fn get_fields(&self) -> &Vec<FinalizedMemberField> {
        return match self {
            FinalizedTypes::Struct(inner, _) => &inner.fields,
            FinalizedTypes::Reference(inner) => inner.get_fields(),
            _ => panic!("Tried to get fields of generic!"),
        };
    }

    pub fn find_method(&self, name: &String) -> Option<Vec<(FinalizedTypes, Arc<FunctionData>)>> {
        return match self {
            FinalizedTypes::Struct(inner, _) => inner
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
        if let FinalizedTypes::Struct(inner, _) = self {
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
            FinalizedTypes::Struct(structure, _) => {
                match &structure.data.chalk_data.as_ref().unwrap() {
                    ChalkData::Struct(types, _) => types.clone(), // skipcq: RS-W1110
                    ChalkData::Trait(types, _, _) => types.clone(), // skipcq: RS-W1110
                }
            }
            FinalizedTypes::Reference(inner) => inner.to_chalk_type(binders),
            FinalizedTypes::Array(inner) => {
                TyKind::Slice(inner.to_chalk_type(binders)).intern(ChalkIr)
            }
            FinalizedTypes::Generic(name, _bounds) => {
                let index = binders.iter().position(|found| *found == name).unwrap();
                TyKind::BoundVar(BoundVar {
                    debruijn: DebruijnIndex::INNERMOST,
                    index,
                })
                .intern(ChalkIr)
            }
            FinalizedTypes::GenericType(inner, bounds) => {
                if let TyKind::Adt(id, _) = inner.to_chalk_type(binders).data(ChalkIr).kind {
                    let mut generic_args = Vec::default();
                    for arg in bounds {
                        generic_args
                            .push(GenericArgData::Ty(arg.to_chalk_type(binders)).intern(ChalkIr));
                    }
                    // Returns the structure with the correct substitutions from bounds for generic types.
                    return TyKind::Adt(id, Substitution::from_iter(ChalkIr, generic_args))
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
            FinalizedTypes::Struct(structure, _) => structure,
            FinalizedTypes::Reference(inner) => inner.inner_struct(),
            FinalizedTypes::GenericType(inner, _) => inner.inner_struct(),
            _ => panic!("Tried to get inner struct of invalid type! {:?}", self),
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
    // skipcq: RS-R1000
    pub fn of_type_sync(
        &self,
        other: &FinalizedTypes,
        syntax: Option<Arc<Mutex<Syntax>>>,
    ) -> (
        bool,
        Option<Pin<Box<dyn Future<Output = bool> + Send + Sync>>>,
    ) {
        return match self {
            FinalizedTypes::Struct(found, _original) => match other {
                FinalizedTypes::Struct(other_struct, _) => {
                    if found == other_struct {
                        (true, None)
                    } else if found.data.name.contains('<')
                        && found.data.name.split('<').next().unwrap() == other_struct.data.name
                    {
                        (true, None)
                    } else if is_modifier(other.inner_struct().data.modifiers, Modifier::Trait) {
                        if syntax.is_none() {
                            return (false, None);
                        }
                        return (
                            false,
                            Some(Box::pin(TypeWaiter {
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
                    let mut fails = Vec::default();
                    if generics.len() != other_generics.len() {
                        let (result, future) = base.of_type_sync(other_base, syntax.clone());
                        if !result {
                            if let Some(found) = future {
                                fails.push(found);
                            } else {
                                return (false, None);
                            }
                        }
                    }

                    for i in 0..generics.len() {
                        let (result, future) =
                            generics[i].of_type_sync(&other_generics[i], syntax.clone());
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
                FinalizedTypes::Struct(_, _) => base.of_type_sync(other, syntax),
                // References are ignored for type checking.
                FinalizedTypes::Reference(inner) => self.of_type_sync(inner, syntax),
                FinalizedTypes::Array(_) => (false, None),
            },
            // References are ignored for type checking.
            FinalizedTypes::Reference(referencing) => referencing.of_type_sync(other, syntax),
            FinalizedTypes::Generic(_, bounds) => match other {
                FinalizedTypes::Generic(_, other_bounds) => {
                    let mut outer_fails: Vec<Pin<Box<dyn Future<Output = bool> + Send + Sync>>> =
                        Vec::default();
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
                FinalizedTypes::Struct(_, _)
                | FinalizedTypes::GenericType(_, _)
                | FinalizedTypes::Array(_) => {
                    let mut fails = Vec::default();
                    for bound in bounds {
                        let (result, failure) = bound.of_type_sync(other, syntax.clone());
                        if result {
                            return (true, None);
                        } else if let Some(found) = failure {
                            fails.push(found);
                        }
                    }
                    return if !fails.is_empty() {
                        (false, Some(Box::pin(Self::join(fails))))
                    } else {
                        (false, None)
                    };
                } //T: u64 is Number
                  //Number is T: u64
            },
        };
    }

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
    #[async_recursion]
    pub async fn resolve_generic(
        &self,
        other: &FinalizedTypes,
        syntax: &Arc<Mutex<Syntax>>,
        generics: &mut HashMap<String, FinalizedTypes>,
        mut bounds_error: ParsingError,
    ) -> Result<(), ParsingError> {
        match self {
            FinalizedTypes::Generic(name, bounds) => {
                // Check for bound errors.
                for bound in bounds {
                    if !other.of_type(bound, syntax.clone()).await {
                        bounds_error.message += &*format!(" {} and {}", other, bound);
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
                    base.resolve_generic(other_base, syntax, generics, bounds_error.clone())
                        .await?;

                    for i in 0..bounds.len() {
                        bounds[i]
                            .resolve_generic(
                                &other_bounds[i],
                                syntax,
                                generics,
                                bounds_error.clone(),
                            )
                            .await?;
                    }
                }
            }
            // Ignore references.
            FinalizedTypes::Reference(inner) => {
                return inner
                    .resolve_generic(other, syntax, generics, bounds_error)
                    .await;
            }
            FinalizedTypes::Array(inner) => {
                let mut other = other;
                // Ignore references.
                while let FinalizedTypes::Reference(inner) = other {
                    other = inner;
                }
                // Check on the inner type.
                if let FinalizedTypes::Array(other) = other {
                    return inner
                        .resolve_generic(other, syntax, generics, bounds_error)
                        .await;
                }
                return Err(bounds_error);
            }
            _ => {}
        }
        return Ok(());
    }

    /// Degenerics the type by replacing all generics with their solidified value.
    #[async_recursion]
    pub async fn degeneric(
        &mut self,
        generics: &HashMap<String, FinalizedTypes>,
        syntax: &Arc<Mutex<Syntax>>,
        mut none_error: ParsingError,
        bounds_error: ParsingError,
    ) -> Result<(), ParsingError> {
        return match self {
            FinalizedTypes::Generic(name, bounds) => {
                if let Some(found) = generics.get(name) {
                    // This should never trip, but it's a sanity check.
                    for bound in bounds {
                        if !found.of_type(bound, syntax.clone()).await {
                            return Err(bounds_error);
                        }
                    }
                    self.clone_from(found);
                    Ok(())
                } else {
                    none_error.message = format!(
                        "{}: {} and {:?}",
                        none_error.message,
                        self,
                        generics.keys().collect::<Vec<_>>()
                    );
                    Err(none_error)
                }
            }
            FinalizedTypes::GenericType(base, bounds) => {
                base.degeneric(generics, syntax, none_error.clone(), bounds_error.clone())
                    .await?;
                let mut found = Vec::default();
                for bound in bounds {
                    bound
                        .degeneric(generics, syntax, none_error.clone(), bounds_error.clone())
                        .await?;
                    found.push(bound.clone());
                }
                *self = base.flatten(&mut found, syntax).await?;
                Ok(())
            }
            FinalizedTypes::Reference(inner) => {
                inner
                    .degeneric(generics, syntax, none_error, bounds_error)
                    .await
            }
            FinalizedTypes::Array(inner) => {
                inner
                    .degeneric(generics, syntax, none_error, bounds_error)
                    .await
            }
            _ => Ok(()),
        };
    }

    pub fn unflatten(&self) -> FinalizedTypes {
        return match self {
            FinalizedTypes::Struct(_, original) => original
                .clone()
                .map_or_else(|| self.clone(), |inner| *inner),
            FinalizedTypes::Reference(inner) => inner.unflatten(),
            _ => self.clone(),
        };
    }

    /// Flattens GenericTypes into a Structure, degenericing them.
    #[async_recursion]
    pub async fn flatten(
        &self,
        generics: &Vec<FinalizedTypes>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> Result<FinalizedTypes, ParsingError> {
        return match self {
            FinalizedTypes::Struct(found, _) => {
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
                        // skipcq: RS-W1070
                        data = locked.structures.types.get(&name).unwrap().clone();
                    }
                    let base = AsyncDataGetter::new(syntax.clone(), data).await;
                    Ok(FinalizedTypes::Struct(
                        base.clone(),
                        Some(Box::new(FinalizedTypes::GenericType(
                            Box::new(FinalizedTypes::Struct(found.clone(), None)),
                            generics.clone(),
                        ))),
                    ))
                } else {
                    // Clone the type and add the new type to the structures.
                    let mut other = StructData::clone(&found.data);
                    other.name.clone_from(&name);

                    // Update the structure's functions
                    for function in &mut other.functions {
                        let mut temp = FunctionData::clone(function);
                        temp.name = format!("{}::{}", name, temp.name.split("::").last().unwrap());
                        let temp = Arc::new(temp);
                        *function = temp;
                    }

                    let arc_other;
                    {
                        let mut locked = syntax.lock().unwrap();
                        other.set_id(locked.structures.sorted.len() as u64);
                        arc_other = Arc::new(other);
                        locked
                            .structures
                            .types
                            .insert(name.clone(), arc_other.clone());
                        locked.structures.sorted.push(arc_other.clone());
                    }
                    // Get the FinalizedStruct and degeneric it.
                    let mut data = FinalizedStruct::clone(
                        AsyncDataGetter::new(syntax.clone(), found.data.clone())
                            .await
                            .deref(),
                    );
                    data.data.clone_from(&arc_other);
                    data.degeneric(generics, syntax).await?;
                    let data = Arc::new(data);
                    // Add the flattened type to the
                    let mut locked = syntax.lock().unwrap();
                    if let Some(wakers) = locked.structures.wakers.remove(&data.data.name) {
                        for waker in wakers {
                            waker.wake();
                        }
                    }

                    locked.structures.data.insert(arc_other, data.clone());
                    Ok(FinalizedTypes::Struct(
                        data.clone(),
                        Some(Box::new(FinalizedTypes::GenericType(
                            Box::new(FinalizedTypes::Struct(found.clone(), None)),
                            generics.clone(),
                        ))),
                    ))
                }
            }
            FinalizedTypes::Reference(other) => other.flatten(generics, syntax).await,
            FinalizedTypes::Array(inner) => inner.flatten(generics, syntax).await,
            FinalizedTypes::Generic(_, _) => panic!("Unresolved generic!"),
            FinalizedTypes::GenericType(base, effects) => base.flatten(effects, syntax).await,
        };
    }

    pub fn name(&self) -> String {
        return match self {
            FinalizedTypes::Struct(structs, _) => structs.data.name.clone(),
            FinalizedTypes::Reference(structs) => structs.name(),
            FinalizedTypes::Array(inner) => format!("[{}]", inner.name()),
            FinalizedTypes::Generic(name, _) => {
                panic!("Generics should never be named, tried to get {}", name)
            }
            FinalizedTypes::GenericType(_, _) => panic!("Generics should never be named"),
        };
    }

    pub fn name_safe(&self) -> Option<String> {
        return match self {
            FinalizedTypes::Struct(structs, _) => Some(structs.data.name.clone()),
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
            FinalizedTypes::Struct(structure, _) => write!(f, "{}", structure.data.name),
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
            other.name_safe().map_or(false, |other| inner == other)
        });
    }
}
