use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use crate::{ParsingError, ProcessManager};
use crate::async_util::{FunctionGetter, NameResolver, StructureGetter};
use crate::function::Function;
use crate::r#struct::Struct;
use crate::types::Types;

pub struct Syntax {
    pub structures: HashMap<String, Arc<Struct>>,
    pub functions: HashMap<String, Arc<Function>>,
    pub structure_wakers: HashMap<String, Vec<Waker>>,
    pub function_wakers: HashMap<String, Vec<Waker>>,
    pub finished: bool,
    pub process_manager: Box<dyn ProcessManager>
}

impl Syntax {
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            structures: HashMap::new(),
            functions: HashMap::new(),
            structure_wakers: HashMap::new(),
            function_wakers: HashMap::new(),
            finished: false,
            process_manager
        };
    }

    pub fn finish(&mut self) {
        self.finished = true;
        self.structure_wakers.values().for_each(|wakers| wakers.iter().for_each(|waker| waker.clone().wake()));
        self.function_wakers.values().for_each(|wakers| wakers.iter().for_each(|waker| waker.clone().wake()));
    }

    //noinspection DuplicatedCode I could use a poisonable trait to extract this code but too much work
    pub fn add_struct(&mut self, dupe_error: Option<ParsingError>, structure: Arc<Struct>) {
        if let Some(old) = self.structures.get_mut(&structure.name) {
            if old.poisoned.is_empty() && structure.poisoned.is_empty() {
                unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(dupe_error.unwrap());
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

    //noinspection DuplicatedCode I could use a poisonable trait to extract this code but too much work
    pub fn add_function(&mut self, dupe_error: ParsingError, function: Arc<Function>) {
        if let Some(old) = self.functions.get_mut(&function.name) {
            if old.poisoned.is_empty() && function.poisoned.is_empty() {
                unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(dupe_error);
            } else {
                for poison in &function.poisoned {
                    unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(poison.clone());
                }
            }
        } else {
            self.functions.insert(function.name.clone(), function.clone());
        }
        if let Some(wakers) = self.function_wakers.remove(&function.name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, error: ParsingError, name: String, name_resolver: Box<dyn NameResolver>)
                              -> Result<Arc<Function>, ParsingError> {
        return FunctionGetter::new(syntax, error, name, name_resolver).await;
    }
}