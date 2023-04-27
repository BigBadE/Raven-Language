use std::task::{RawWaker, RawWakerVTable, Waker};
use std::{ptr, thread};
use syntax::syntax::Syntax;

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, set_finish, set_finish, set_finish);

/// Synchronously waits for parsing to stop
pub struct TypeWaiter {
    data: bool,
}

fn set_finish(data: *const ()) {
    unsafe { ptr::write(data as *const bool as *mut bool, true) };
}

fn clone(data: *const ()) -> RawWaker {
    return RawWaker::new(data, &VTABLE);
}

impl TypeWaiter {
    pub fn new(syntax: &mut Syntax, function: &str) -> Self {
        let returning = Self {
            data: false
        };
        let waker = unsafe { Waker::from_raw(clone(&returning.data as *const bool as *const ())) };
        syntax.async_manager.finish.push(waker.clone());
        match syntax.functions.wakers.get_mut(function) {
            Some(wakers) => wakers.push(waker),
            None => {
                syntax.functions.wakers.insert(function.to_string(), vec!(waker));
            }
        }
        return returning;
    }

    pub fn wait(&self) {
        while !&self.data {
            thread::yield_now();
        }
    }
}