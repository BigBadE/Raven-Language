use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::{ParsingError, TopElement};
use crate::syntax::Syntax;
use crate::types::Types;

pub(crate) struct AsyncTypesGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub error: ParsingError,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>,
    pub finished: Option<Arc<T>>,
}

impl<T: TopElement> AsyncTypesGetter<T> {
    pub fn new(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        {
            let mut locked = syntax.lock().unwrap();
            locked.async_manager.locked += 1;
            locked.async_manager.remaining += 1;
            println!("Started looking for {} ({}, {})", getting, locked.async_manager.locked, locked.async_manager.remaining);
        }

        return Self {
            syntax,
            error,
            getting,
            name_resolver,
            finished: None,
        };
    }
}

impl<T: TopElement> Future for AsyncTypesGetter<T> {
    type Output = Result<Arc<T>, ParsingError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        locked.async_manager.locked -= 1;
        println!("Started looking for {} ({}, {})", self.getting, locked.async_manager.locked, locked.async_manager.remaining);

        let name = self.name_resolver.resolve(&self.getting);

        //Look for a structure of that name
        if let Some(found) = T::get_manager(locked.deref_mut()).types.get(name).cloned() {
            self.finished = Some(found.clone());
            locked.async_manager.remaining -= 1;
            println!("Passed for {} ({}, {})", self.getting, locked.async_manager.locked, locked.async_manager.remaining);

            return Poll::Ready(Ok(found));
        }

        check_wake(locked.deref_mut());
        println!("Failed for {} ({}, {})", self.getting, locked.async_manager.locked, locked.async_manager.remaining);

        if locked.async_manager.finished && locked.async_manager.locked >= locked.async_manager.remaining {
            return Poll::Ready(Err(self.error.clone()));
        }

        let targets = T::get_manager(locked.deref_mut());

        //Add a waker for that type
        if let Some(vectors) = targets.wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            targets.wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        return Poll::Pending;
    }
}

pub trait NameResolver: Send + Sync {
    fn resolve<'a>(&'a self, name: &'a String) -> &'a String;

    fn generic(&self, name: &String) -> Option<Types>;

    fn boxed_clone(&self) -> Box<dyn NameResolver>;
}

fn check_wake(locked: &mut Syntax) {
    locked.async_manager.locked += 1;

    //If this is the last running task, fail it all.
    if locked.async_manager.locked == locked.async_manager.remaining && locked.async_manager.finished {
        locked.structures.wakers.values().for_each(|wakers| for waker in wakers {
            waker.wake_by_ref();
        });
        locked.functions.wakers.values().for_each(|wakers| for waker in wakers {
            waker.wake_by_ref();
        });
    }
}