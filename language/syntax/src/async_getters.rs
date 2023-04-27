use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use crate::{ParsingError, Syntax, TopElement};

#[derive(Default)]
pub struct GetterManager {
    //The amount of tasks running.
    pub remaining: usize,
    //The amount of running tasks locked waiting for their waker.
    pub locked: usize,
    //If parsing is finished
    pub finished: bool,
    //Tasks to call when finished
    pub finish: Vec<Waker>,
}

pub struct AsyncGetter<T> where T: TopElement {
    pub types: HashMap<String, Arc<T>>,
    pub wakers: HashMap<String, Vec<Waker>>,
}

impl<T> AsyncGetter<T> where T: TopElement {
    pub fn new() -> Self {
        return Self {
            types: HashMap::new(),
            wakers: HashMap::new(),
        };
    }
}