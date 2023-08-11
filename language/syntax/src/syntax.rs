use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use chalk_integration::interner::ChalkIr;
use chalk_integration::RawId;
use chalk_ir::{Binders, DomainGoal, GenericArg, GenericArgData, Goal, GoalData, Substitution,
               TraitId, TraitRef, TyVariableKind, VariableKind, VariableKinds, WhereClause};
use chalk_recursive::RecursiveSolver;
use chalk_solve::ext::GoalExt;
use chalk_solve::rust_ir::{ImplDatum, ImplDatumBound, ImplType, Polarity};
use chalk_solve::Solver;
use indexmap::IndexMap;
use no_deadlocks::Mutex;
use tokio::runtime::Handle;

use async_recursion::async_recursion;

use crate::{FinishedTraitImplementor, ParsingError, ProcessManager, TopElement, Types};
use crate::async_getters::{AsyncGetter, GetterManager};
use crate::async_util::{AsyncTypesGetter, NameResolver, UnparsedType};
use crate::function::{FinalizedFunction, FunctionData};
use crate::r#struct::{ChalkData, FinalizedStruct, StructData};
use crate::types::FinalizedTypes;

/// The entire program's syntax, including libraries.
pub struct Syntax {
    // The compiling functions
    pub compiling: Arc<HashMap<String, Arc<FinalizedFunction>>>,
    // The compiling structs
    pub strut_compiling: Arc<HashMap<String, Arc<FinalizedStruct>>>,
    // All parsing errors on the entire program
    pub errors: Vec<ParsingError>,
    // All structures in the program
    pub structures: AsyncGetter<StructData>,
    // All functions in the program
    pub functions: AsyncGetter<FunctionData>,
    // All implementations in the program
    pub implementations: Vec<FinishedTraitImplementor>,
    // Stores the async parsing state
    pub async_manager: GetterManager,
    // All operations without namespaces, for example {}+{} or {}/{}
    pub operations: HashMap<String, Vec<Arc<FunctionData>>>,
    // Manages the next steps of compilation after parsing
    pub process_manager: Box<dyn ProcessManager>,
}

impl Syntax {
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            compiling: Arc::new(HashMap::new()),
            strut_compiling: Arc::new(HashMap::new()),
            errors: Vec::new(),
            structures: AsyncGetter::new(),
            functions: AsyncGetter::new(),
            implementations: Vec::new(),
            async_manager: GetterManager::default(),
            operations: HashMap::new(),
            process_manager,
        };
    }

    pub fn finished_impls(&self) -> bool {
        return self.async_manager.finished && self.async_manager.parsing_impls == 0;
    }

    // Sets the syntax to be finished
    pub fn finish(&mut self) {
        if self.async_manager.finished {
            panic!("Tried to finish already-finished syntax!")
        }
        self.async_manager.finished = true;

        for wakers in &mut self.structures.wakers.values() {
            for waker in wakers {
                waker.wake_by_ref();
            }
        }
        for wakers in &mut self.functions.wakers.values() {
            for waker in wakers {
                waker.wake_by_ref();
            }
        }
        self.structures.wakers.clear();
        self.functions.wakers.clear();
    }

    pub fn make_impldatum(generics: &IndexMap<String, Vec<FinalizedTypes>>,
                          first: &FinalizedTypes, second: &FinalizedTypes) -> ImplDatum<ChalkIr> {
        let vec_generics = generics.keys().collect::<Vec<_>>();
        let first = first.to_trait(&vec_generics);
        let mut binders: Vec<VariableKind<ChalkIr>> = Vec::new();
        //TODO figure out where this is used.
        for _value in generics.values() {
            binders.push(VariableKind::Ty(TyVariableKind::General));
        }
        let second = second.to_chalk_type(&vec_generics);
        let data: &[GenericArg<ChalkIr>] = &[GenericArg::new(ChalkIr, GenericArgData::Ty(second.clone()))];
        return ImplDatum {
            polarity: Polarity::Positive,
            binders: Binders::new(VariableKinds::from_iter(ChalkIr, binders), ImplDatumBound {
                trait_ref: TraitRef { trait_id: first.id.clone(), substitution: Substitution::from_iter(ChalkIr, data) },
                where_clauses: vec![],
            }),
            impl_type: ImplType::Local,
            associated_ty_value_ids: vec![],
        }
    }

    pub fn get_implementation(&self, first: &Arc<StructData>, second: &Arc<StructData>) -> Option<Vec<Arc<FunctionData>>> {
        for implementation in &self.implementations {
            if self.solve(&implementation.base.inner_struct().data, first) &&
                self.solve(second, &implementation.target.inner_struct().data) {
                return Some(implementation.functions.clone());
            }
        }
        return None;
    }

    pub fn solve(&self, first: &StructData, second: &StructData) -> bool {
        let types;
        if let ChalkData::Struct(found_type, _) = &first.chalk_data {
            types = found_type.clone();
        } else {
            panic!("Tried to solve with a trait arg!");
        }
        let elements: &[GenericArg<ChalkIr>] = &[GenericArg::new(ChalkIr, GenericArgData::Ty(types))];
        let goal = Goal::new(ChalkIr, GoalData::DomainGoal(DomainGoal::Holds(
            WhereClause::Implemented(TraitRef {
                trait_id: TraitId(RawId { index: second.id as u32 }),
                substitution: Substitution::from_iter(ChalkIr, elements.into_iter())
            })
        )));

        return RecursiveSolver::new(30, 3000, None)
            .solve(self, &goal.into_closed_goal(ChalkIr)).is_some();
    }

    // Adds the top element to the syntax
    pub fn add<T: TopElement + 'static>(syntax: &Arc<Mutex<Syntax>>, handle: &Handle,
                                        resolver: Box<dyn NameResolver>, dupe_error: ParsingError,
                                        adding: Arc<T>, verifying: T::Unfinalized) {
        let mut locked = syntax.lock().unwrap();
        for poison in adding.errors() {
            locked.errors.push(poison.clone());
        }
        if let Some(mut old) = T::get_manager(locked.deref_mut()).types.get_mut(adding.name()).cloned() {
            if adding.errors().is_empty() && adding.errors().is_empty() {
                locked.errors.push(dupe_error.clone());
                unsafe { Arc::get_mut_unchecked(&mut old) }.poison(dupe_error);
            } else {
                //Ignored if one is poisoned
            }
        } else {
            let manager = T::get_manager(locked.deref_mut());
            manager.sorted.push(adding.clone());
            manager.types.insert(adding.name().clone(), adding.clone());
        }

        let name = adding.name().clone();
        if adding.is_operator() {
            //Only functions can be operators. This will break if something else is.
            //These is no better way to do this because Rust.
            let adding: Arc<FunctionData> = unsafe { std::mem::transmute(adding.clone()) };

            let name = adding.name().split("::").last().unwrap().to_string();
            let name = format!("{}${}", name, locked.operations.get(&name).map(|found| found.len()).unwrap_or(0));
            match locked.operations.get_mut(&name) {
                Some(found) => found.push(adding),
                None => {
                    locked.operations.insert(name.clone(), vec!(adding));
                }
            }
        }

        if let Some(wakers) = T::get_manager(locked.deref_mut()).wakers.remove(&name) {
            for waker in wakers {
                waker.wake();
            }
        }

        let process_manager = locked.process_manager.cloned();
        handle.spawn(T::verify(verifying, syntax.clone(), resolver, process_manager));
    }

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

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                              getting: String, operation: bool, name_resolver: Box<dyn NameResolver>) -> Result<Arc<FunctionData>, ParsingError> {
        return AsyncTypesGetter::new_func(syntax, error, getting, operation, name_resolver).await;
    }

    #[async_recursion]
    pub async fn get_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                            getting: String, name_resolver: Box<dyn NameResolver>) -> Result<Types, ParsingError> {
        if getting.as_bytes()[0] == b'[' {
            return Ok(Types::Array(Box::new(Self::get_struct(syntax, error, getting[1..getting.len() - 1].to_string(),
                                                             name_resolver).await?)));
        }
        if let Some(found) = name_resolver.generic(&getting) {
            let mut bounds = Vec::new();
            for bound in found {
                bounds.push(Self::parse_type(syntax.clone(), error.clone(),
                                             name_resolver.boxed_clone(), bound).await?);
            }
            return Ok(Types::Generic(getting, bounds));
        }
        return Ok(Types::Struct(AsyncTypesGetter::new_struct(syntax, error, getting, name_resolver).await?));
    }

    #[async_recursion]
    pub async fn parse_type(syntax: Arc<Mutex<Syntax>>, error: ParsingError, resolver: Box<dyn NameResolver>,
                            types: UnparsedType) -> Result<Types, ParsingError> {
        let temp = match types {
            UnparsedType::Basic(name) =>
                Syntax::get_struct(syntax, Self::swap_error(error, &name), name, resolver).await,
            UnparsedType::Generic(name, args) => {
                let mut generics = Vec::new();
                for arg in args {
                    generics.push(Self::parse_type(syntax.clone(),
                                                   error.clone(), resolver.boxed_clone(), arg).await?);
                }
                Ok(Types::GenericType(Box::new(
                    Self::parse_type(syntax, error, resolver, *name).await?),
                                      generics))
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

pub trait Compiler<T> {
    /// Compiles the main function and returns the main runner.
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>) -> Result<Option<T>, Vec<ParsingError>>;
}