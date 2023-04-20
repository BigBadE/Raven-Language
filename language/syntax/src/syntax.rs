use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use crate::{ParsingError, ProcessManager};
use crate::async_util::{FunctionGetter, NameResolver};
use crate::function::Function;
use crate::r#struct::Struct;

pub struct Syntax {
    pub errors: Vec<ParsingError>,
    pub structures: HashMap<String, Arc<Struct>>,
    pub functions: HashMap<String, Arc<Function>>,
    pub structure_wakers: HashMap<String, Vec<Waker>>,
    pub function_wakers: HashMap<String, Vec<Waker>>,
    pub done_parsing: bool,
    pub remaining: usize,
    pub finish: Vec<Waker>,
    pub process_manager: Box<dyn ProcessManager>
}

impl Syntax {
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            errors: Vec::new(),
            structures: HashMap::new(),
            functions: HashMap::new(),
            structure_wakers: HashMap::new(),
            function_wakers: HashMap::new(),
            done_parsing: false,
            remaining: 0,
            finish: Vec::new(),
            process_manager
        };
    }

    pub fn finish(&mut self) {
        self.done_parsing = true;
        self.structure_wakers.values().for_each(|wakers| wakers.iter().for_each(|waker| waker.clone().wake()));
        self.function_wakers.values().for_each(|wakers| wakers.iter().for_each(|waker| waker.clone().wake()));
    }

    //noinspection DuplicatedCode I could use a poisonable trait to extract this code but too much work
    pub fn add_struct(&mut self, dupe_error: Option<ParsingError>, structure: Arc<Struct>) {
        self.remaining -= 1;
        for poison in &structure.poisoned {
            self.errors.push(poison.clone());
        }
        if let Some(old) = self.structures.get_mut(&structure.name) {
            if old.poisoned.is_empty() && structure.poisoned.is_empty() {
                self.errors.push(dupe_error.as_ref().unwrap().clone());
                unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(dupe_error.unwrap());
            } else {
                //Ignored if one is poisoned
            }
        } else {
            if structure.poisoned.is_empty() {
                self.process_manager.verify_struct(&structure);
            }
            self.structures.insert(structure.name.clone(), structure.clone());
        }
        if let Some(wakers) = self.structure_wakers.remove(&structure.name) {
            for waker in wakers {
                waker.wake();
            }
        }
        if self.remaining == 0 {
            for waker in &self.finish {
                waker.wake_by_ref();
            }
        }
    }

    //noinspection DuplicatedCode I could use a poisonable trait to extract this code but too much work
    pub fn add_function(&mut self, decrement: bool, dupe_error: ParsingError, function: Arc<Function>) {
        if decrement {
            self.remaining -= 1;
        }

        for poison in &function.poisoned {
            self.errors.push(poison.clone());
        }
        if let Some(old) = self.functions.get_mut(&function.name) {
            if old.poisoned.is_empty() && function.poisoned.is_empty() {
                self.errors.push(dupe_error.clone());
                unsafe { Arc::get_mut_unchecked(old) }.poisoned.push(dupe_error);
            } else {
                //Ignore if one is poisoned
            }
        } else {
            if function.poisoned.is_empty() {
                self.process_manager.verify_func(&function);
            }
            self.functions.insert(function.name.clone(), function.clone());
        }
        if let Some(wakers) = self.function_wakers.remove(&function.name) {
            for waker in wakers {
                waker.wake();
            }
        }
        if self.remaining == 0 {
            for waker in &self.finish {
                waker.wake_by_ref();
            }
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, error: ParsingError, name: String, name_resolver: Box<dyn NameResolver>)
                              -> Result<Arc<Function>, ParsingError> {
        return FunctionGetter::new(syntax, error, name, name_resolver).await;
    }
}