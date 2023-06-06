use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use crate::function::Function;
use crate::syntax::Syntax;
use crate::TopElement;
use crate::types::Types;

/// The async manager, just stores the basic post-parsing tasks.
#[derive(Default)]
pub struct GetterManager {
    //If parsing is finished
    pub finished: bool,
    //Tasks to call when finished
    pub finish: Vec<Waker>,
}

/// Generic async type manager, holds the types and the wakers requiring those types.
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

/// This getter waits until an implementation becomes available for the given type (if one exists).
/// If one doesn't exist, an Err will be returned with nothing in it.
pub struct ImplementationGetter {
    syntax: Arc<Mutex<Syntax>>,
    testing: Types,
    target: Types,
}

impl ImplementationGetter {
    pub fn new(syntax: Arc<Mutex<Syntax>>, testing: Types, target: Types) -> Self {
        return ImplementationGetter {
            syntax,
            testing,
            target,
        };
    }
}

impl Future for ImplementationGetter {
    type Output = Result<Vec<Arc<Function>>, ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        if let Poll::Ready(result) = Future::poll(Pin::new(
            &mut locked.process_manager.of_types(&self.target, &self.testing, &self.syntax)), cx) {
            if let Some(found) = result {
                return Poll::Ready(Ok(found.clone()));
            }
        } else {
            return Poll::Pending;
        }

        if locked.async_manager.finished {
            return Poll::Ready(Err(()));
        }

        locked.process_manager.add_impl_waiter(cx.waker().clone(), self.testing.clone());
        return Poll::Pending;
    }
}