use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use crate::{ParsingError, ProcessManager};
use crate::async_util::{FunctionGetter, NameResolver, StructureGetter};
use crate::function::Function;
use crate::r#struct::Struct;

pub struct Syntax {
    pub structures: HashMap<String, Arc<Struct>>,
    pub static_functions: HashMap<String, Arc<Function>>,
    pub structure_wakers: HashMap<String, Vec<Waker>>,
    pub function_wakers: HashMap<String, Vec<Waker>>,
    pub manager: Box<dyn ProcessManager>,
    pub finished: bool,
}

impl Syntax {
    pub fn new(manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            structures: HashMap::new(),
            static_functions: HashMap::new(),
            structure_wakers: HashMap::new(),
            function_wakers: HashMap::new(),
            manager,
            finished: false,
        };
    }

    pub fn finish(&mut self) {
        self.finished = true;
        self.structure_wakers.values().for_each(|wakers| wakers.iter().for_each(|waker| waker.clone().wake()));
        self.function_wakers.values().for_each(|wakers| wakers.iter().for_each(|waker| waker.clone().wake()));
    }

    pub fn add_struct(&mut self, structure: Arc<Struct>) {
        if let Some(old) = self.structures.get_mut(&structure.name) {
            if old.poisoned.is_empty() && structure.poisoned.is_empty() {
                unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(ParsingError::new((0, 0), (0, 0), 
                                                    "Duplicate struct name!".to_string()));
            } else {
                for poison in &structure.poisoned {
                    unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(poison.clone());
                }
            }
        } else {
            self.structures.insert(structure.name.clone(), structure.clone());
        }
        if let Some(wakers) = self.structure_wakers.remove(&structure.name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn get_struct(syntax: Arc<Mutex<Syntax>>, name: String, name_resolver: Box<dyn NameResolver>)
                            -> Result<Arc<Struct>, ParsingError> {
        return StructureGetter::new(syntax, name, name_resolver).await;
    }

    pub fn add_function(&mut self, function: Arc<Function>) {
        if let Some(old) = self.static_functions.get_mut(&function.name) {
            if old.poisoned.is_empty() && function.poisoned.is_empty() {
                unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(ParsingError::new((0, 0), (0, 0),
                                                                                       "Duplicate function name!".to_string()));
            } else {
                for poison in &function.poisoned {
                    unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(poison.clone());
                }
            }
        } else {
            self.static_functions.insert(function.name.clone(), function.clone());
        }
        if let Some(wakers) = self.function_wakers.remove(&function.name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, name: String, name_resolver: Box<dyn NameResolver>)
                              -> Result<Arc<Function>, ParsingError> {
        return FunctionGetter::new(syntax, name, name_resolver).await;
    }
}