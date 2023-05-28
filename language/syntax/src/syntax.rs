use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

use async_recursion::async_recursion;

use crate::{get_all_names, ParsingError, ProcessManager, TopElement, Types};
use crate::async_getters::{AsyncGetter, GetterManager};
use crate::async_util::{AsyncTypesGetter, NameResolver, UnparsedType};
use crate::function::Function;
use crate::r#struct::Struct;

pub struct Syntax {
    pub errors: Vec<ParsingError>,
    pub structures: AsyncGetter<Struct>,
    pub functions: AsyncGetter<Function>,
    pub async_manager: GetterManager,
    pub operations: HashMap<String, Vec<Arc<Function>>>,
    pub process_manager: Box<dyn ProcessManager>,
}

impl Syntax {
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            errors: Vec::new(),
            structures: AsyncGetter::new(),
            functions: AsyncGetter::new(),
            async_manager: GetterManager::default(),
            operations: HashMap::new(),
            process_manager,
        };
    }

    pub fn finish(&mut self) {
        self.async_manager.finished = true;
    }

    pub async fn add<T: TopElement>(syntax: &Arc<Mutex<Syntax>>, dupe_error: ParsingError, mut adding: Arc<T>) {
        let mut process_manager = syntax.lock().unwrap().process_manager.cloned();

        unsafe { Arc::get_mut_unchecked(&mut adding) }.verify(syntax, process_manager.deref_mut()).await;

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

        let mut split = get_all_names(adding.name());

        if adding.is_operator() {
            //Only functions can be operators. This will break if something else is.
            //These is no better way to do this because Rust.
            let adding: Arc<Function> = unsafe { std::mem::transmute(adding) };

            let name = adding.name().split("::").last().unwrap().to_string();
            let name = format!("{}${}", name, locked.operations.get(&name).map(|found| found.len()).unwrap_or(0));
            match locked.operations.get_mut(&name) {
                Some(found) => found.push(adding),
                None => {
                    locked.operations.insert(name.clone(), vec!(adding));
                }
            }
        }

        for name in split {
            if let Some(wakers) = T::get_manager(locked.deref_mut()).wakers.remove(&name) {
                for waker in wakers {
                    waker.wake();
                }
            }
        }
    }

    pub fn add_poison<T: TopElement>(&mut self, element: Arc<T>) {
        for poison in element.errors() {
            self.errors.push(poison.clone());
        }

        let getter = T::get_manager(self);
        if getter.types.get_mut(element.name()).is_none() {
            getter.types.insert(element.name().clone(), element.clone());
        }

        for name in get_all_names(element.name()) {
            if let Some(wakers) = getter.wakers.remove(&name) {
                for waker in wakers {
                    waker.wake();
                }
            }
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                              getting: String, operation: bool, name_resolver: Box<dyn NameResolver>) -> Result<Arc<Function>, ParsingError> {
        return AsyncTypesGetter::new_func(syntax, error, getting, operation, name_resolver).await;
    }

    pub async fn get_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                            getting: String, name_resolver: Box<dyn NameResolver>) -> Result<Types, ParsingError> {
        if let Some(found) = name_resolver.generic(&getting) {
            let mut bounds = Vec::new();
            for bound in found {
                //Async recursion isn't sync, but futures are implicitly sync.
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
        return match types {
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
    }

    fn swap_error(error: ParsingError, new_type: &String) -> ParsingError {
        let mut error = error.clone();
        error.message = format!("Unknown type {}!", new_type);
        return error;
    }
}