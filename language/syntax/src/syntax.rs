use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use crate::async_util::{FunctionGetter, NameResolver, StructureGetter};
use crate::function::Function;
use crate::ProcessManager;
use crate::r#struct::Struct;
use crate::types::Types;

pub struct Syntax {
    pub structures: HashMap<String, Arc<Struct>>,
    pub static_functions: HashMap<String, Arc<Function>>,
    pub structure_wakers: HashMap<String, Vec<Waker>>,
    pub function_wakers: HashMap<String, Vec<Waker>>,
    pub manager: Box<dyn ProcessManager>
}

impl Syntax {
    pub fn new(manager: Box<dyn ProcessManager>) -> Self {
        return Self {
            structures: HashMap::new(),
            static_functions: HashMap::new(),
            structure_wakers: HashMap::new(),
            function_wakers: HashMap::new(),
            manager
        }
    }

    pub fn add_struct(&mut self, structure: Arc<Struct>) {
        self.structures.insert(structure.name.clone(), structure.clone());
        if let Some(wakers) = self.structure_wakers.remove(&structure.name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn get_struct(syntax: Arc<Mutex<Syntax>>, name: String, name_resolver: Box<dyn NameResolver>) -> Arc<Struct> {
        return StructureGetter::new(syntax, name, name_resolver).await;
    }

    pub fn add_function(&mut self, function: Arc<Function>) {
        self.static_functions.insert(function.name.clone(), function.clone());
        if let Some(wakers) = self.function_wakers.remove(&function.name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    pub async fn get_function(syntax: Arc<Mutex<Syntax>>, name: String, name_resolver: Box<dyn NameResolver>) -> Arc<Function> {
        return FunctionGetter::new(syntax, name, name_resolver).await;
    }
}