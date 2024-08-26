use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;

use chalk_ir::{
    Binders, DomainGoal, GenericArg, GenericArgData, Goal, GoalData, Substitution, TraitId, TraitRef, TyVariableKind,
    VariableKind, VariableKinds, WhereClause,
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
use data::tokens::Span;
pub use data::Main;

use crate::async_util::{AsyncStructImplGetter, AsyncTypesGetter, NameResolver, UnparsedType};
use crate::chalk_interner::ChalkIr;
use crate::errors::{ErrorSource, ParsingMessage};
use crate::program::function::{FinalizedFunction, FunctionData};
use crate::program::r#struct::{FinalizedStruct, StructData, BOOL, F32, F64, I16, I32, I64, I8, STR, U16, U32, U64, U8};
use crate::program::types::FinalizedTypes;
use crate::top_element_manager::{GetterManager, TopElementManager};
use crate::{
    is_modifier, Attribute, FinishedStructImplementor, FinishedTraitImplementor, Modifier, ParsingError, ProcessManager,
    TopElement, Types,
};

/// The entire program's syntax. Contains all the data passed to every step of the program.
/// This program is usually in a Mutex lock, which prevents multiple functions from reading/writing
/// to it at the same time.
/// This is done so that the whole compiler can safely run multithreaded.
pub struct Syntax {
    /// The compiled functions.
    pub compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
    /// The compiling functions, accessed from the compiler.
    pub compiling_wakers: HashMap<String, Vec<Waker>>,
    /// The generic functions in the program, uses the compiling wakers.
    pub generics: Arc<DashMap<String, Arc<FinalizedFunction>>>,
    /// The compiling structs, accessed from the compiler.
    pub strut_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
    /// All parsing errors on the entire program
    pub errors: Vec<ParsingError>,
    /// All structures in the program
    pub structures: TopElementManager<StructData>,
    /// All functions in the program
    pub functions: TopElementManager<FunctionData>,
    /// All implementations of a trait in the program
    pub implementations: Vec<Arc<FinishedTraitImplementor>>,
    /// All implementations of a struct in the program
    pub struct_implementations: HashMap<FinalizedTypes, Vec<Arc<FinishedStructImplementor>>>,
    /// The parsing state
    pub async_manager: GetterManager,
    /// All operations, for example Add or Multiply.
    pub operations: HashMap<String, Arc<StructData>>,
    /// Wakers waiting for a specific operation to be finished parsing. Will never deadlock
    /// because types are added before they're finalized.
    pub operation_wakers: HashMap<String, Vec<Waker>>,
    /// Manages the next steps of compilation after parsing
    pub process_manager: Box<dyn ProcessManager>,
}

impl Syntax {
    /// Constructs a new syntax with internal types.
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            compiling: Arc::new(DashMap::default()),
            generics: Arc::new(DashMap::default()),
            compiling_wakers: HashMap::default(),
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
            struct_implementations: HashMap::default(),
            async_manager: GetterManager::default(),
            operations: HashMap::default(),
            operation_wakers: HashMap::default(),
            process_manager,
        };
    }

    /// Adds a function to the compiling list
    pub async fn add_compiling(
        process_manager: Box<dyn ProcessManager>,
        function: Arc<FinalizedFunction>,
        syntax: &Arc<Mutex<Syntax>>,
        generic: bool,
    ) {
        let mut waker: Option<Waker> = None;
        {
            let mut locked = syntax.lock();
            locked.compiling_wakers.remove(&function.data.name).into_iter().flatten().for_each(Waker::wake);

            if function.data.name != locked.async_manager.target {
                // Prevent duplicates from empty trait methods
                if function.code.expressions.len() == 0
                    && ((locked.compiling.contains_key(&function.data.name) && !generic)
                        || (locked.generics.contains_key(&function.data.name) && generic))
                {
                    return;
                }

                if !generic {
                    locked.compiling.insert(function.data.name.clone(), function);
                    return;
                }
                locked.generics.insert(function.data.name.clone(), function.clone());

                // TODO figure out generic traits
                // If the function is in a trait, it can't be generic, so it gets automatically compiled
                if is_modifier(function.data.modifiers, Modifier::Trait) {
                    locked.compiling.insert(function.data.name.clone(), function);
                }
                return;
            }

            if !function.generics.is_empty() || !function.fields.is_empty() {
                panic!("Invalid main function! Make sure your main function is the top function in your file");
            }

            if generic {
                locked.generics.insert(function.data.name.clone(), function.clone());
            } else {
                locked.compiling.insert(function.data.name.clone(), function.clone());
            }
            waker.clone_from(&locked.async_manager.target_waker);
        }

        if generic {
            process_manager.degeneric_code(Arc::new(function.to_codeless()), syntax).await;
        }
        if let Some(found) = waker {
            found.wake();
        }
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

        self.structures.wakers.values().flatten().for_each(Waker::wake_by_ref);
        self.structures.wakers.clear();

        self.functions.wakers.values().flatten().for_each(Waker::wake_by_ref);
        self.functions.wakers.clear();

        self.operation_wakers.values().flatten().for_each(Waker::wake_by_ref);
        self.operation_wakers.clear();

        self.async_manager.impl_waiters.iter().for_each(Waker::wake_by_ref);
        self.async_manager.impl_waiters.clear();
    }

    /// Converts an implementation into a Chalk ImplDatum. This allows implementations to be used
    /// in the solve method, which calls on the Chalk library.
    pub fn make_impldatum(
        generics: &IndexMap<String, FinalizedTypes>,
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
        let data: &[GenericArg<ChalkIr>] = &[GenericArg::new(ChalkIr, GenericArgData::Ty(second.clone()))];
        return ImplDatum {
            polarity: Polarity::Positive,
            binders: Binders::new(
                VariableKinds::from_iter(ChalkIr, binders),
                ImplDatumBound {
                    trait_ref: TraitRef { trait_id: first.id.clone(), substitution: Substitution::from_iter(ChalkIr, data) },
                    where_clauses: vec![],
                },
            ),
            impl_type: ImplType::Local,
            associated_ty_value_ids: vec![],
        };
    }

    /// Finds an implementation method for the given trait.
    /// Can return multiple implementations in cases where generic bounds allow for different implementations of the same thing
    pub async fn get_implementation_methods(
        syntax: &Arc<Mutex<Syntax>>,
        struct_type: &FinalizedTypes,
        trait_type: &FinalizedTypes,
    ) -> Option<Vec<(Arc<FinishedTraitImplementor>, Vec<Arc<FunctionData>>)>> {
        let mut output = Vec::default();
        let implementations = {
            let locked = syntax.lock();
            locked.implementations.clone()
        };

        for implementation in &implementations {
            if trait_type.of_type_sync(&implementation.target, None).0
                && struct_type.of_type(&implementation.base, syntax.clone()).await
            {
                output.push((implementation.clone(), implementation.functions.clone()));
            }
        }
        return if output.is_empty() { None } else { Some(output) };
    }

    /// Recursively solves if a type is a generic type by checking if the target type matches all the bounds.
    fn solve_nonstruct_types(&self, target_type: &FinalizedTypes, checking: &FinalizedTypes) -> Option<bool> {
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
            FinalizedTypes::Reference(inner, _) => {
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
        // Check to make sure the type is a basic program, Chalk can't handle any other types.
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
        let first_ty = first.inner_struct().data.chalk_data.get_ty().clone();

        let elements: &[GenericArg<ChalkIr>] = &[GenericArg::new(ChalkIr, GenericArgData::Ty(first_ty))];
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
        let value = RecursiveSolver::new(30, 3000, None).solve(self, &goal.into_closed_goal(ChalkIr)).is_some();
        return value;
    }

    pub fn add_function(syntax: &Arc<Mutex<Syntax>>, adding: &mut Arc<FunctionData>) {
        let mut locked = syntax.lock();
        locked.add(adding);
    }

    pub fn add_struct(syntax: &Arc<Mutex<Syntax>>, adding: &mut Arc<StructData>) {
        let mut locked = syntax.lock();
        locked.add(adding);
        if !adding.is_operator() {
            return;
        }
        // Gets the name of the operation, or errors if there isn't one.
        let name = if let Attribute::String(_, name) = Attribute::find_attribute("operation", &adding.attributes).unwrap() {
            name.replace("{+}", "{}").clone()
        } else {
            locked.errors.push(ParsingError::new(Span::default(), ParsingMessage::StringAttribute));
            return;
        };

        // Checks if there is a duplicate of that operation.
        if locked.operations.contains_key(&name) {
            locked.errors.push(adding.get_span().make_error(ParsingMessage::DuplicateStructure));
        }

        locked.operations.insert(name.clone(), adding.clone());

        // Wakes every waker waiting for that operation.
        let Some(wakers) = locked.operation_wakers.get(&name) else {
            return;
        };

        for waker in wakers {
            waker.wake_by_ref();
        }
    }

    /// Adds the element to the syntax
    fn add<T: TopElement + 'static>(&mut self, adding: &mut Arc<T>) {
        /*TODO fix duplicate catching
        if T::get_manager(locked.deref_mut()).types.contains_key(adding.name()) {
            locked.errors.push(adding.get_span().make_error("Duplicate type!"));
        }*/

        let manager = T::get_manager(self);
        manager.add_type(adding.clone());

        // Add any poisons to the syntax errors list.
        for poison in adding.errors() {
            self.errors.push(poison.clone());
        }

        // Wakes every waker waiting for that type.
        if let Some(wakers) = T::get_manager(self).wakers.remove(adding.name()) {
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
        getting: (String, Span),
        name_resolver: Box<dyn NameResolver>,
        not_trait: bool,
    ) -> Result<Arc<FunctionData>, ParsingError> {
        return AsyncTypesGetter::new(syntax, getting, name_resolver, not_trait).await;
    }

    /// Gets the implementation of a structure
    pub async fn get_struct_impl(
        syntax: Arc<Mutex<Syntax>>,
        getting: FinalizedTypes,
    ) -> Vec<Arc<FinishedStructImplementor>> {
        return AsyncStructImplGetter::new(syntax.clone(), getting).await;
    }

    /// Asynchronously gets a struct, or returns the error if that struct isn't found.
    #[async_recursion]
    pub async fn get_struct(
        syntax: Arc<Mutex<Syntax>>,
        getting: (String, Span),
        name_resolver: Box<dyn NameResolver>,
        mut resolved_generics: Vec<String>,
    ) -> Result<Types, ParsingError> {
        let (name, span) = getting;

        if name.starts_with('&') {
            return Ok(Types::Reference(
                Box::new(Self::get_struct(syntax, (name[1..].to_string(), span), name_resolver, resolved_generics).await?),
                vec![],
            ));
        }
        // Checks if the type is a generic type
        if let Some(generic_bounds) = name_resolver.generic(&name) {
            if resolved_generics.contains(&name) {
                // If the generic is recursive, for example "T: Add<E, T>", then ignore the bounds since they're irrelevant in the second recursive case
                return Ok(Types::Generic(name, vec![]));
            }
            resolved_generics.push(name.clone());
            let mut bounds = vec![];
            for bound in generic_bounds {
                bounds.push(
                    Self::parse_type(syntax.clone(), name_resolver.boxed_clone(), bound, resolved_generics.clone()).await?,
                );
            }
            return Ok(Types::Generic(name, bounds));
        }

        if name.contains('<') {
            return Ok(Self::parse_bounds(name.as_bytes(), &syntax, &span, &*name_resolver).await?.remove(0));
        }
        return Ok(Types::Struct(AsyncTypesGetter::new(syntax, (name, span), name_resolver, false).await?));
    }

    /// Parses generic bounds on a type, returning the length parsed and the types found.
    /// TODO should probably be mostly moved to the tokenizer
    #[async_recursion]
    async fn parse_bounds(
        input: &[u8],
        syntax: &Arc<Mutex<Syntax>>,
        error: &Span,
        name_resolver: &dyn NameResolver,
    ) -> Result<Vec<Types>, ParsingError> {
        let mut last = 0;
        let mut found = Vec::default();
        let mut i = 0;
        while i < input.len() {
            match input[i] {
                b'<' => {
                    let first = String::from_utf8_lossy(&input[last..i]);
                    let bounds = Self::parse_bounds(&input[i + 1..], syntax, error, name_resolver).await?;
                    let first = Self::get_struct(
                        syntax.clone(),
                        (first.to_string(), error.clone()),
                        name_resolver.boxed_clone(),
                        vec![],
                    )
                    .await?;
                    found.push(Types::GenericType(Box::new(first), bounds));
                    return Ok(found);
                }
                b',' => {
                    let getting = String::from_utf8_lossy(&input[last..i]);
                    found.push(
                        Self::get_struct(
                            syntax.clone(),
                            (getting.to_string(), error.clone()),
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
                            (first.to_string(), error.clone()),
                            name_resolver.boxed_clone(),
                            vec![],
                        )
                        .await?,
                    );
                    return Ok(found);
                }
                b' ' => last = i + 1,
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
        resolver: Box<dyn NameResolver>,
        types: UnparsedType,
        resolved_generics: Vec<String>,
    ) -> Result<Types, ParsingError> {
        let temp = match types.clone() {
            UnparsedType::Basic(span, name) => Syntax::get_struct(syntax, (name, span), resolver, resolved_generics).await,
            UnparsedType::Generic(name, args) => {
                let mut generics = Vec::default();
                for arg in args {
                    generics.push(
                        Self::parse_type(syntax.clone(), resolver.boxed_clone(), arg, resolved_generics.clone()).await?,
                    );
                }

                Ok(Types::GenericType(
                    Box::new(Self::parse_type(syntax, resolver, *name, resolved_generics).await?),
                    generics,
                ))
            }
        };
        return temp;
    }
}

/// The compiler
#[async_trait]
pub trait Compiler<T> {
    /// Compiles the target function and returns the main runner.
    /// Waits for the receiver before calling any of the code
    async fn compile(&self, receiver: Receiver<()>, syntax: &Arc<Mutex<Syntax>>) -> Option<T>;
}
