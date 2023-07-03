use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

use async_recursion::async_recursion;

use crate::{Attribute, ParsingError, ProcessManager, TopElement, Types};
use crate::async_getters::{AsyncGetter, GetterManager};
use crate::async_util::{AsyncTypesGetter, NameResolver, UnparsedType};
use crate::function::{FinalizedFunction, FunctionData};
use crate::r#struct::{FinalizedStruct, StructData};
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
    pub implementations: HashMap<FinalizedTypes, (FinalizedTypes, Vec<Attribute>, Vec<Arc<FunctionData>>)>,
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
            implementations: HashMap::new(),
            async_manager: GetterManager::default(),
            operations: HashMap::new(),
            process_manager,
        };
    }

    // Sets the syntax to be finished
    pub fn finish(&mut self) {
        if self.async_manager.finished {
            panic!("Tried to finish already-finished syntax!")
        }
        self.async_manager.finished = true;
    }

    /// Checks if the given target type matches the base type.
    /// Can false negative unless parsing is finished.
    pub async fn of_types(base: &FinalizedTypes, target: &FinalizedTypes, syntax: &Arc<Mutex<Syntax>>) -> Option<Vec<Arc<FunctionData>>> {
        let implementations = syntax.lock().unwrap().implementations.clone();
        for (implementor, (other, _, functions)) in implementations {
            if base.of_type(&implementor, syntax).await &&
                other.of_type(&target, syntax).await {
                return Some(functions);
            }
        }

        return None;
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
            T::get_manager(locked.deref_mut()).types.insert(adding.name().clone(), adding.clone());
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

    pub async fn get_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                            getting: String, name_resolver: Box<dyn NameResolver>) -> Result<Types, ParsingError> {
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

pub type Output = i64;

pub trait Compiler {
    /// Compiles the main function and returns the main runner.
    fn compile(&self, syntax: &Arc<Mutex<Syntax>>) -> Result<Option<Output>, Vec<ParsingError>>;
}