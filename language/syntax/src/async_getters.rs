use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;
use crate::TopElement;

/// The async manager, just stores basic information about the current parsing state.
#[derive(Default)]
pub struct GetterManager {
    //If parsing non-impls is finished
    pub finished: bool,
    //How many impls are still being parsed, which is done async and not tied to finished
    pub parsing_impls: u32,
    //Impl waiters, which are woken whenever an impl finishes parsing.
    pub impl_waiters: Vec<Waker>
}

/// Generic async type manager, holds the types and the wakers requiring those types.
/// Wakers are used to allow tasks to wait for a type to be parsed and added
pub struct AsyncGetter<T> where T: TopElement {
    //Types and their data, added immediately after parsing
    pub types: HashMap<String, Arc<T>>,
    //A list of data sorted by the data's ID. Guaranteed to be in ID order.
    pub sorted: Vec<Arc<T>>,
    //Data sorted by its finalized type, which contains the finalized code. Added after finalization.
    pub data: HashMap<Arc<T>, Arc<T::Finalized>>,
    //Wakers waiting on a type to be added to the types hashmap, waked after the type is added to types
    pub wakers: HashMap<String, Vec<Waker>>,
}

impl<T> AsyncGetter<T> where T: TopElement {
    pub fn new() -> Self {
        return Self {
            types: HashMap::new(),
            sorted: Vec::new(),
            data: HashMap::new(),
            wakers: HashMap::new(),
        };
    }

    //Creates the getter with a list of sorted types already, used for internal types declared in the compiler
    pub fn with_sorted(sorted: Vec<Arc<T>>) -> Self {
        return Self {
            types: HashMap::new(),
            sorted,
            data: HashMap::new(),
            wakers: HashMap::new(),
        };
    }
}