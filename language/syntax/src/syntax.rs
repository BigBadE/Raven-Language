use std::collections::HashMap;
use std::mem;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;

use chalk_ir::{
    Binders, DomainGoal, GenericArg, GenericArgData, Goal, GoalData, Substitution, TraitId,
    TraitRef, TyVariableKind, VariableKind, VariableKinds, WhereClause,
};
use chalk_recursive::RecursiveSolver;
use chalk_solve::ext::GoalExt;
use chalk_solve::rust_ir::{ImplDatum, ImplDatumBound, ImplType, Polarity};
use chalk_solve::Solver;
use dashmap::DashMap;
use indexmap::IndexMap;
use tokio::sync::mpsc::Receiver;

use async_recursion::async_recursion;
use async_trait::async_trait;
// Re-export main
pub use data::Main;

use crate::async_util::{AsyncTypesGetter, NameResolver, UnparsedType};
use crate::chalk_interner::ChalkIr;
use crate::function::{FinalizedFunction, FunctionData};
use crate::r#struct::{
    FinalizedStruct, StructData, BOOL, F32, F64, I16, I32, I64, I8, STR, U16, U32, U64, U8,
};
use crate::top_element_manager::{GetterManager, TopElementManager};
use crate::types::FinalizedTypes;
use crate::{
    is_modifier, Attribute, FinishedTraitImplementor, Modifier, ParsingError, ProcessManager,
    TopElement, Types,
};

/// The entire program's syntax. Contains all the data passed to every step of the program.
/// This structure is usually in a mutex lock, which prevents multiple functions from reading/writing
/// to it at the same time.
/// This is done so that the whole compiler can safely run multithreaded.
pub struct Syntax {
    // The compiling functions, accessed from the compiler.
    pub compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
    // Compiling wakers
    pub compiling_wakers: Vec<Waker>,
    // The compiling structs, accessed from the compiler..
    pub strut_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
    // All parsing errors on the entire program
    pub errors: Vec<ParsingError>,
    // All structures in the program
    pub structures: TopElementManager<StructData>,
    // All functions in the program
    pub functions: TopElementManager<FunctionData>,
    // All implementations in the program
    pub implementations: Vec<FinishedTraitImplementor>,
    // The parsing state
    pub async_manager: GetterManager,
    // All operations, for example Add or Multiply.
    pub operations: HashMap<String, Arc<StructData>>,
    // Wakers waiting for a specific operation to be finished parsing. Will never deadlock
    // because types are added before they're finalized.
    pub operation_wakers: HashMap<String, Vec<Waker>>,
    // Manages the next steps of compilation after parsing
    pub process_manager: Box<dyn ProcessManager>,
}

impl Syntax {
    /// Constructs a new syntax with internal types.
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            compiling: Arc::new(DashMap::default()),
            compiling_wakers: Vec::default(),
            strut_compiling: Arc::new(DashMap::default()),
            errors: Vec::default(),
            functions: TopElementManager::default(),
            structures: TopElementManager::with_sorted(vec![
                I64.data.clone(),
                I32.data.clone(),
                I16.data.clone(),
                I8.data.clone(),
                F64.data.clone(),
                F32.data.clone(),
                U64.data.clone(),
                U32.data.clone(),
                U16.data.clone(),
                U8.data.clone(),
                BOOL.data.clone(),
                STR.data.clone(),
            ]),
            implementations: Vec::default(),
            async_manager: GetterManager::default(),
            operations: HashMap::default(),
            operation_wakers: HashMap::default(),
            process_manager,
        };
    }

    /// Checks if the implementations are finished parsing.
    pub fn finished_impls(&self) -> bool {
        return self.async_manager.finished && self.async_manager.parsing_impls == 0;
    }

    /// Sets the syntax to be finished, calling all wakers so non-existent functions can be detected.
    pub fn finish(&mut self) {
        if self.async_manager.finished {
            panic!("Tried to finish already-finished syntax!")
        }
        self.async_manager.finished = true;

        let mut keys = Vec::default();
        self.structures
            .wakers
            .keys()
            .for_each(|inner| keys.push(inner.clone()));
        for key in &keys {
            for waker in self.structures.wakers.remove(key).unwrap() {
                waker.wake_by_ref();
            }
        }

        keys.clear();
        self.functions
            .wakers
            .keys()
            .for_each(|inner| keys.push(inner.clone()));
        for key in &keys {
            for waker in self.functions.wakers.remove(key).unwrap() {
                waker.wake_by_ref();
            }
        }

        keys.clear();
        self.operation_wakers
            .keys()
            .for_each(|inner| keys.push(inner.clone()));
        for key in &keys {
            for waker in self.operation_wakers.remove(key).unwrap() {
                waker.wake_by_ref();
            }
        }
    }

    /// Converts an implementation into a Chalk ImplDatum. This allows implementations to be used
    /// in the solve method, which calls on the Chalk library.
    pub fn make_impldatum(
        generics: &IndexMap<String, Vec<FinalizedTypes>>,
        first: &FinalizedTypes,
        second: &FinalizedTypes,
    ) -> ImplDatum<ChalkIr> {
        let vec_generics = generics.keys().collect::<Vec<_>>();
        let first = first.to_chalk_trait(&vec_generics);
        let mut binders: Vec<VariableKind<ChalkIr>> = Vec::default();
        // We resolve generics ourselves, but Chalk needs to know about them.
        for _value in generics.values() {
            binders.push(VariableKind::Ty(TyVariableKind::General));
        }
        let second = second.to_chalk_type(&vec_generics);
        let data: &[GenericArg<ChalkIr>] =
            &[GenericArg::new(ChalkIr, GenericArgData::Ty(second.clone()))];
        return ImplDatum {
            polarity: Polarity::Positive,
            binders: Binders::new(
                VariableKinds::from_iter(ChalkIr, binders),
                ImplDatumBound {
                    trait_ref: TraitRef {
                        trait_id: first.id.clone(),
                        substitution: Substitution::from_iter(ChalkIr, data),
                    },
                    where_clauses: vec![],
                },
            ),
            impl_type: ImplType::Local,
            associated_ty_value_ids: vec![],
        };
    }

    /// Finds an implementation method for the given trait.
    pub fn get_implementation_methods(
        &self,
        implementing_trait: &FinalizedTypes,
        implementor_struct: &FinalizedTypes,
    ) -> Option<Vec<Arc<FunctionData>>> {
        let mut output = Vec::default();
        for implementation in &self.implementations {
            if implementation.target.inner_struct().data == implementor_struct.inner_struct().data
                && (implementing_trait
                    .of_type_sync(&implementation.base, None)
                    .0
                    || self.solve(&implementing_trait, &implementation.base))
            {
                for function in &implementation.functions {
                    output.push(function.clone());
                }
            }
        }
        return if output.is_empty() {
            None
        } else {
            Some(output)
        };
    }

    /// Recursively solves if a type is a generic type by checking if the target type matches all the bounds.
    fn solve_nonstruct_types(
        &self,
        target_type: &FinalizedTypes,
        checking: &FinalizedTypes,
    ) -> Option<bool> {
        return match target_type {
            FinalizedTypes::Generic(_, bounds) => {
                // If a single bound fails, than the type isn't of the generic type.
                for bound in bounds {
                    if !self.solve(bound, checking) && !self.solve(checking, bound) {
                        return Some(false);
                    }
                }
                Some(true)
            }
            FinalizedTypes::Array(inner) => {
                let mut checking = checking;
                // Unwrap references because references don't matter for type checking.
                if let FinalizedTypes::Reference(inner_type) = checking {
                    checking = inner_type;
                }
                if let FinalizedTypes::Array(other) = checking {
                    // Check the inner type if both are generics
                    self.solve_nonstruct_types(inner, other)
                } else {
                    Some(false)
                }
            }
            FinalizedTypes::Reference(inner) => {
                // References are unwrapped and the inner is checked.
                self.solve_nonstruct_types(inner, checking)
            }
            _ => None,
        };
    }

    /// Solves if the first type is the second type, either if they are equal or if it is within the
    /// bounds or has an implementation for it.
    /// May not be correct if the syntax isn't finished parsing implementations, check Syntax::finished_impls.
    pub fn solve(&self, first: &FinalizedTypes, second: &FinalizedTypes) -> bool {
        // Check to make sure the type is a basic structure, Chalk can't handle any other types.
        // u64 is T: Add<E, T>
        if let Some(inner) = self.solve_nonstruct_types(second, first) {
            return inner;
        }

        // T: Add<E, T> is u64
        if let Some(inner) = self.solve_nonstruct_types(first, second) {
            return inner;
        }

        let second_ty = &second.inner_struct().data;
        if !is_modifier(second_ty.modifiers, Modifier::Trait) {
            return false;
        }
        let first_ty = first
            .inner_struct()
            .data
            .chalk_data
            .as_ref()
            .unwrap()
            .get_ty()
            .clone();

        let elements: &[GenericArg<ChalkIr>] =
            &[GenericArg::new(ChalkIr, GenericArgData::Ty(first_ty))];
        // Construct a goal asking if the first type is implemented by the second type.
        let goal = Goal::new(
            ChalkIr,
            GoalData::DomainGoal(DomainGoal::Holds(WhereClause::Implemented(TraitRef {
                trait_id: TraitId(second_ty.id as u32),
                substitution: Substitution::from_iter(ChalkIr, elements.into_iter()),
            }))),
        );

        // Tell Chalk to solve it, ignoring any overflows.
        // TODO add a cache for speed?
        let value = RecursiveSolver::new(30, 3000, None)
            .solve(self, &goal.into_closed_goal(ChalkIr))
            .is_some();
        return value;
    }

    /// Adds the element to the syntax
    pub fn add<T: TopElement + Eq + 'static>(
        syntax: &Arc<Mutex<Syntax>>,
        dupe_error: ParsingError,
        adding: &Arc<T>,
    ) {
        let mut locked = syntax.lock().unwrap();
        unsafe {
            // Safety: add blocks the method which contains the other arc references, and they aren't shared across threads
            // yet, so this is safe.
            Arc::get_mut_unchecked(&mut adding.clone()).set_id(
                locked
                    .structures
                    .sorted
                    .iter()
                    .position(|found| &found.name == adding.name())
                    .unwrap_or_else(|| locked.structures.sorted.len()) as u64,
            );
        }

        // Add any poisons to the syntax errors list.
        for poison in adding.errors() {
            locked.errors.push(poison.clone());
        }

        // Checks if a type with the same name is already in the async manager.
        if let Some(mut old) = T::get_manager(locked.deref_mut())
            .types
            .get_mut(adding.name())
            .cloned()
        {
            if adding.errors().is_empty() && adding.errors().is_empty() {
                // Add a duplication error to the original type.
                locked.errors.push(dupe_error.clone());
                unsafe { Arc::get_mut_unchecked(&mut old) }.poison(dupe_error.clone());
            } else {
                // Ignored if one is poisoned
            }
        } else {
            let manager = T::get_manager(locked.deref_mut());
            // Don't want to add duplicates of internal types.
            if !manager.sorted.contains(adding) {
                manager.sorted.push(Arc::clone(adding));
            }

            manager
                .types
                .insert(adding.name().clone(), Arc::clone(adding));
        }

        let name = adding.name().clone();
        if adding.is_operator() {
            //Downcasts the generic type to be a StructData.
            //Only traits can be operators. This will break if something else is.
            //These is no better way to do this because Rust doesn't allow downcasting generics.
            // skipcq: RS-W1117
            let adding: Arc<StructData> = unsafe { mem::transmute(adding.clone()) };

            // Gets the name of the operation, or errors if there isn't one.
            let name = if let Attribute::String(_, name) =
                Attribute::find_attribute("operation", &adding.attributes).unwrap()
            {
                name.replace("{+}", "{}").clone()
            } else {
                let mut error = ParsingError::empty();
                error.message = format!("Expected a string with attribute operator!");
                locked.errors.push(error);
                return;
            };

            // Checks if there is a duplicate of that operation.
            if locked.operations.contains_key(&name) {
                locked.errors.push(dupe_error);
            }

            locked.operations.insert(name.clone(), adding);

            // Wakes every waker waiting for that operation.
            if let Some(wakers) = locked.operation_wakers.get(&name) {
                for waker in wakers {
                    waker.wake_by_ref();
                }
            }
        }

        // Wakes every waker waiting for that type.
        if let Some(wakers) = T::get_manager(locked.deref_mut()).wakers.remove(&name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    /// Adds a poisoned type, which means it errored and shouldn't be checked for completeness.
    pub fn add_poison<T: TopElement>(&mut self, element: Arc<T>) {
        for poison in element.errors() {
            self.errors.push(poison.clone());
        }

        let getter = T::get_manager(self);
        if getter.types.get_mut(element.name()).is_none() {
            getter.sorted.push(element.clone());
            getter.types.insert(element.name().clone(), element.clone());
        }

        if let Some(wakers) = getter.wakers.remove(element.name()) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    /// Asynchronously gets a function, or returns the error if that function isn't found.
    pub async fn get_function(
        syntax: Arc<Mutex<Syntax>>,
        error: ParsingError,
        getting: String,
        name_resolver: Box<dyn NameResolver>,
        not_trait: bool,
    ) -> Result<Arc<FunctionData>, ParsingError> {
        return AsyncTypesGetter::new(syntax, error, getting, name_resolver, not_trait).await;
    }

    /// Asynchronously gets a struct, or returns the error if that struct isn't found.
    #[async_recursion]
    pub async fn get_struct(
        syntax: Arc<Mutex<Syntax>>,
        error: ParsingError,
        getting: String,
        name_resolver: Box<dyn NameResolver>,
        mut resolved_generics: Vec<String>,
    ) -> Result<Types, ParsingError> {
        // Handles arrays by removing the brackets and getting the inner type
        if getting.as_bytes()[0] == b'[' {
            return Ok(Types::Array(Box::new(
                Self::get_struct(
                    syntax,
                    error,
                    getting[1..getting.len() - 1].to_string(),
                    name_resolver,
                    resolved_generics,
                )
                .await?,
            )));
        }

        // Checks if the type is a generic type
        if let Some(found) = name_resolver.generic(&getting) {
            let mut bounds = Vec::default();
            // Get all the generic's bounds.
            if !resolved_generics.contains(&getting) {
                resolved_generics.push(getting.clone());
                for bound in found {
                    bounds.push(
                        Self::parse_type(
                            syntax.clone(),
                            error.clone(),
                            name_resolver.boxed_clone(),
                            bound,
                            resolved_generics.clone(),
                        )
                        .await?,
                    );
                }
            }

            return Ok(Types::Generic(getting, bounds));
        }

        if getting.contains('<') {
            return Ok(
                Self::parse_bounds(getting.as_bytes(), &syntax, &error, &*name_resolver)
                    .await?
                    .1
                    .remove(0),
            );
        }
        return Ok(Types::Struct(
            AsyncTypesGetter::new(syntax, error, getting, name_resolver, false).await?,
        ));
    }

    #[async_recursion]
    async fn parse_bounds(
        input: &[u8],
        syntax: &Arc<Mutex<Syntax>>,
        error: &ParsingError,
        name_resolver: &dyn NameResolver,
    ) -> Result<(usize, Vec<Types>), ParsingError> {
        let mut last = 0;
        let mut found = Vec::default();
        let mut i = 0;
        while i < input.len() {
            match input[i] {
                b'<' => {
                    let first = String::from_utf8_lossy(&input[last..i]);
                    let (size, bounds) =
                        Self::parse_bounds(&input[i + 1..], syntax, error, name_resolver).await?;
                    let first = Self::get_struct(
                        syntax.clone(),
                        error.clone(),
                        first.to_string(),
                        name_resolver.boxed_clone(),
                        vec![],
                    )
                    .await?;
                    found.push(Types::GenericType(Box::new(first), bounds));
                    return Ok((i + size, found));
                }
                b',' => {
                    let getting = String::from_utf8_lossy(&input[last..i]);
                    found.push(
                        Self::get_struct(
                            syntax.clone(),
                            error.clone(),
                            getting.to_string(),
                            name_resolver.boxed_clone(),
                            vec![],
                        )
                        .await?,
                    );
                    last = i + 1;
                }
                b'>' => {
                    let first = String::from_utf8_lossy(&input[last..i]);
                    found.push(
                        Self::get_struct(
                            syntax.clone(),
                            error.clone(),
                            first.to_string(),
                            name_resolver.boxed_clone(),
                            vec![],
                        )
                        .await?,
                    );
                    return Ok((i, found));
                }
                _ => {}
            }
            i += 1;
        }

        panic!("Expected a < in this bound!")
    }

    /// Parses an UnparsedType into a Types
    #[async_recursion]
    pub async fn parse_type(
        syntax: Arc<Mutex<Syntax>>,
        error: ParsingError,
        resolver: Box<dyn NameResolver>,
        types: UnparsedType,
        resolved_generics: Vec<String>,
    ) -> Result<Types, ParsingError> {
        let temp = match types.clone() {
            UnparsedType::Basic(name) => {
                Syntax::get_struct(
                    syntax,
                    Self::swap_error(error, &name),
                    name,
                    resolver,
                    resolved_generics,
                )
                .await
            }
            UnparsedType::Generic(name, args) => {
                let mut generics = Vec::default();
                for arg in args {
                    generics.push(
                        Self::parse_type(
                            syntax.clone(),
                            error.clone(),
                            resolver.boxed_clone(),
                            arg,
                            resolved_generics.clone(),
                        )
                        .await?,
                    );
                }
                Ok(Types::GenericType(
                    Box::new(
                        Self::parse_type(syntax, error, resolver, *name, resolved_generics).await?,
                    ),
                    generics,
                ))
            }
        };
        return temp;
    }

    fn swap_error(error: ParsingError, new_type: &String) -> ParsingError {
        let mut error = error.clone();
        error.message = format!("Unknown type {}!", new_type);
        return error;
    }
}

#[async_trait]
pub trait Compiler<T> {
    /// Compiles the target function and returns the main runner.
    /// Waits for the receiver before calling any of the code
    async fn compile(&self, receiver: Receiver<()>, syntax: &Arc<Mutex<Syntax>>) -> Option<T>;
}
