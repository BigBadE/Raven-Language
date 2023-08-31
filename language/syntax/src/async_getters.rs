use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;
use crate::TopElement;

/// The async manager, just stores the basic post-parsing tasks.
#[derive(Default)]
pub struct GetterManager {
    //If parsing is finished
    pub finished: bool,
    //Parsing impls
    pub parsing_impls: u32,
    //Impl waiters
    pub impl_waiters: Vec<Waker>,
    //Tasks to call when finished
    pub finish: Vec<Waker>,
}

/// Generic async type manager, holds the types and the wakers requiring those types.
pub struct AsyncGetter<T> where T: TopElement {
    pub types: HashMap<String, Arc<T>>,
    pub sorted: Vec<Arc<T>>,
    pub data: HashMap<Arc<T>, Arc<T::Finalized>>,
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

    pub fn with_sorted(sorted: Vec<Arc<T>>) -> Self {
        return Self {
            types: HashMap::new(),
            sorted,
            data: HashMap::new(),
            wakers: HashMap::new(),
        };
    }
}