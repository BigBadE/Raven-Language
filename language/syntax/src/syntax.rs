use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

use crate::{ParsingError, ProcessManager, TopElement, Types};
use crate::async_getters::{AsyncGetter, GetterManager};
use crate::async_util::{AsyncTypesGetter, NameResolver};
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
        if let Some(wakers) = T::get_manager(locked.deref_mut()).wakers.remove(adding.name()) {
            for waker in wakers {
                waker.wake();
            }
        }

        if adding.is_operator() {
            //Only functions can be operators. This will break if something else is.
            //These is no better way to do this because Rust.
            let adding: Arc<Function> = unsafe { std::mem::transmute(adding) };

            let name = adding.name().split("::").last().unwrap().to_string();
            match locked.operations.get_mut(&name) {
                Some(found) => found.push(adding),
                None => {
                    locked.operations.insert(name.clone(), vec!(adding));
                }
            }

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
        if let Some(wakers) = getter.wakers.remove(element.name()) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                              getting: String, name_resolver: Box<dyn NameResolver>) -> Result<Arc<Function>, ParsingError> {
        return AsyncTypesGetter::new(syntax, error, getting, name_resolver).await;
    }

    pub async fn get_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError,
                              getting: String, name_resolver: Box<dyn NameResolver>) -> Result<Types, ParsingError> {
        if let Some(found) = name_resolver.generic(&getting) {
            return Ok(found);
        }
        return Ok(Types::Struct(AsyncTypesGetter::new(syntax, error, getting, name_resolver).await?));
    }
}