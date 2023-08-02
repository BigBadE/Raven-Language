use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc; use no_deadlocks::Mutex;
use std::task::{Context, Poll, Waker};
use crate::function::FunctionData;
use crate::syntax::Syntax;
use crate::{ParsingError, TopElement};
use crate::types::FinalizedTypes;

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
    pub data: HashMap<Arc<T>, Arc<T::Finalized>>,
    pub wakers: HashMap<String, Vec<Waker>>,
}

impl<T> AsyncGetter<T> where T: TopElement {
    pub fn new() -> Self {
        return Self {
            types: HashMap::new(),
            data: HashMap::new(),
            wakers: HashMap::new(),
        };
    }
}

/// This getter waits until an implementation becomes available for the given type (if one exists).
/// If one doesn't exist, an Err will be returned with nothing in it.
pub struct ImplementationGetter {
    syntax: Arc<Mutex<Syntax>>,
    testing: FinalizedTypes,
    target: FinalizedTypes,
    error: ParsingError,
    index: usize
}

impl ImplementationGetter {
    pub fn new(syntax: Arc<Mutex<Syntax>>, testing: FinalizedTypes, target: FinalizedTypes, error: ParsingError) -> Self {
        return ImplementationGetter {
            syntax,
            testing,
            target,
            error,
            index: 0
        };
    }
    
    pub fn new_with_index(syntax: Arc<Mutex<Syntax>>, testing: FinalizedTypes, target: FinalizedTypes, error: ParsingError, index: usize) -> Self {
        return ImplementationGetter {
            syntax,
            testing,
            target,
            error,
            index
        };
    }
}

impl Future for ImplementationGetter {
    type Output = Result<Vec<Arc<FunctionData>>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safety:
        // Pin requires the type to be Unpin, which Future isn't.
        // This could be fixed with Box::pin, but Future::poll requires a Pin<&mut impl Future>.
        // Luckily, all pointers are implicitly Unpin (because moving a pointer doesn't change the
        // object it points to).
        // Sadly, Rust fails to detect this. The futures crate could be used to avoid the unsafe,
        // but it seems unreasonable to use the entire crate for one macro.
        let mut future = Syntax::of_types(&self.target, &self.testing, &self.syntax, self.index);
        if let Poll::Ready(result) = Future::poll(unsafe { Pin::new_unchecked(&mut future)}, cx) {
            let locked = self.syntax.lock().unwrap();
            if let Some(found) = result {
                return Poll::Ready(Ok(found.clone()));
            } else if locked.async_manager.finished && locked.async_manager.parsing_impls == 0 {
                println!("Failed! {} vs {}", self.target, self.testing);
                return Poll::Ready(Err(self.error.clone()));
            }
        }

        self.syntax.lock().unwrap().async_manager.impl_waiters.push(cx.waker().clone());
        return Poll::Pending;
    }
}