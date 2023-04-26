use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use crate::{is_modifier, Modifier, ParsingError, ProcessManager};
use crate::async_util::{FunctionGetter, NameResolver};
use crate::function::Function;
use crate::r#struct::Struct;

pub struct Syntax {
    pub errors: Vec<ParsingError>,
    pub structures: HashMap<String, Arc<Struct>>,
    pub functions: HashMap<String, Arc<Function>>,
    pub structure_wakers: HashMap<String, Vec<Waker>>,
    pub function_wakers: HashMap<String, Vec<Waker>>,
    pub operations: HashMap<String, Vec<Arc<Function>>>,
    //The amount of tasks running.
    pub remaining: usize,
    //If parsing is finished
    pub finished: bool,
    //The amount of running tasks locked waiting for their waker.
    pub locked: usize,
    pub finish: Vec<Waker>,
    pub process_manager: Box<dyn ProcessManager>,
}

impl Syntax {
    pub fn new(process_manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            errors: Vec::new(),
            structures: HashMap::new(),
            functions: HashMap::new(),
            structure_wakers: HashMap::new(),
            function_wakers: HashMap::new(),
            operations: HashMap::new(),
            remaining: 0,
            finished: false,
            locked: 0,
            finish: Vec::new(),
            process_manager,
        };
    }

    pub fn finish(&mut self) {
        self.finished = true;
    }

    //noinspection DuplicatedCode I could use a poisonable trait to extract this code but too much work
    pub async fn add_struct(syntax: &Arc<Mutex<Syntax>>, decrement: bool, dupe_error: ParsingError, structure: Arc<Struct>) {
        let mut process_manager = None;

        {
            let mut locked = syntax.lock().unwrap();
            if decrement {
                locked.remaining -= 1;
            }
            for poison in &structure.poisoned {
                locked.errors.push(poison.clone());
            }
            if let Some(mut old) = locked.structures.get_mut(&structure.name).cloned() {
                if old.poisoned.is_empty() && structure.poisoned.is_empty() {
                    locked.errors.push(dupe_error.clone());
                    unsafe { Arc::get_mut_unchecked(&mut old) }.poisoned.push(dupe_error);
                } else {
                    //Ignored if one is poisoned
                }
            } else {
                locked.structures.insert(structure.name.clone(), structure.clone());
            }
            if let Some(wakers) = locked.structure_wakers.remove(&structure.name) {
                for waker in wakers {
                    waker.wake();
                }
            }

            if structure.poisoned.is_empty() {
                process_manager = Some(locked.process_manager.cloned());
            }
        }

        if let Some(process_manager) = process_manager {
            process_manager.verify_struct(structure, syntax).await;
        }
    }

    pub fn add_poison_struct(&mut self, decrement: bool, structure: Arc<Struct>) {
        if decrement {
            self.remaining -= 1;
        }

        for poison in &structure.poisoned {
            self.errors.push(poison.clone());
        }

        if self.structures.get_mut(&structure.name).is_none() {
            self.structures.insert(structure.name.clone(), structure.clone());
        }
        if let Some(wakers) = self.structure_wakers.remove(&structure.name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    //noinspection DuplicatedCode I could use a poisonable trait to extract this code but too much work
    pub async fn add_function(syntax: &Arc<Mutex<Syntax>>, decrement: bool, dupe_error: ParsingError, function: Arc<Function>) {
        let mut process_manager = None;

        {
            let mut locked = syntax.lock().unwrap();
            if decrement {
                locked.remaining -= 1;
            }

            for poison in &function.poisoned {
                locked.errors.push(poison.clone());
            }
            if let Some(mut old) = locked.functions.get_mut(&function.name).cloned() {
                if old.poisoned.is_empty() && function.poisoned.is_empty() {
                    locked.errors.push(dupe_error.clone());
                    unsafe { Arc::get_mut_unchecked(&mut old) }.poisoned.push(dupe_error);
                }
                //Ignore if one is poisoned
            } else {
                locked.functions.insert(function.name.clone(), function.clone());
            }

            if is_modifier(function.modifiers, Modifier::Operation) {
                let operation = function.name.split("::").last().unwrap();

                match locked.operations.get_mut(operation) {
                    Some(found) => found.push(function.clone()),
                    None => {
                        locked.operations.insert(operation.to_string(), vec!(function.clone()));
                    }
                }
                if let Some(wakers) = locked.function_wakers.remove(operation) {
                    for waker in wakers {
                        waker.wake();
                    }
                }
            }

            if let Some(wakers) = locked.function_wakers.remove(&function.name) {
                for waker in wakers {
                    waker.wake();
                }
            }

            if function.poisoned.is_empty() {
                process_manager = Some(locked.process_manager.cloned());
            }
        }

        if let Some(process_manager) = process_manager {
            process_manager.verify_func(function, syntax).await
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, operation: bool, error: ParsingError,
                              name: String, name_resolver: Box<dyn NameResolver>)
                              -> Result<Arc<Function>, ParsingError> {
        return FunctionGetter::new(syntax, operation, error, name, name_resolver).await;
    }
}