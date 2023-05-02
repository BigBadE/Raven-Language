use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;
use crate::TopElement;

#[derive(Default)]
pub struct GetterManager {
    //If parsing is finished
    pub finished: bool,
    //Tasks to call when finished
    pub finish: Vec<Waker>,
}

pub struct AsyncGetter<T> where T: TopElement {
    pub types: HashMap<String, Arc<T>>,
    pub wakers: HashMap<String, Vec<Waker>>,
    pub parsing: Vec<String>
}

impl<T> AsyncGetter<T> where T: TopElement {
    pub fn new() -> Self {
        return Self {
            types: HashMap::new(),
            wakers: HashMap::new(),
            parsing: Vec::new()
        };
    }
}