use std::task::{RawWaker, RawWakerVTable, Waker};
use std::{ptr, thread};
use std::sync::{Arc, Mutex};
use syntax::syntax::Syntax;

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, set_finish, set_finish, set_finish);

/// Synchronously waits for parsing to stop
pub struct TypeWaiter {
    syntax: Arc<Mutex<Syntax>>,
    function: String,
    data: bool
}

fn set_finish(data: *const ()) {
    unsafe { ptr::write(data as *const bool as *mut bool, true) };
}

fn clone(data: *const ()) -> RawWaker {
    return RawWaker::new(data, &VTABLE);
}

impl TypeWaiter {
    pub fn new(syntax: &Arc<Mutex<Syntax>>, function: &str) -> Self {
        let returning = Self {
            syntax: syntax.clone(),
            function: function.to_string(),
            data: false
        };
        let waker = unsafe { Waker::from_raw(clone(&returning.data as *const bool as *const ())) };
        let mut lock = syntax.lock().unwrap();
        lock.async_manager.finish.push(waker.clone());
        match lock.functions.wakers.get_mut(function) {
            Some(wakers) => wakers.push(waker),
            None => {
                lock.functions.wakers.insert(function.to_string(), vec!(waker));
            }
        }
        return returning;
    }

    pub fn wait(&self) {
        while !self.syntax.lock().unwrap().functions.types.contains_key(&self.function) {
            thread::yield_now();
        }
    }
}