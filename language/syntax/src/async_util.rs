use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::{ParsingError, TopElement};
use crate::function::Function;
use crate::r#struct::Struct;
use crate::syntax::Syntax;

pub(crate) struct AsyncTypesGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub error: ParsingError,
    pub getting: String,
    pub operation: bool,
    pub name_resolver: Box<dyn NameResolver>,
    pub finished: Option<Arc<T>>,
}

impl AsyncTypesGetter<Function> {
    pub fn new_func(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, operation: bool, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            operation,
            name_resolver,
            finished: None,
        };
    }
}

impl AsyncTypesGetter<Struct> {
    pub fn new_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            operation: false,
            name_resolver,
            finished: None,
        };
    }
}

impl Future for AsyncTypesGetter<Function> {
    type Output = Result<Arc<Function>, ParsingError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        let name = self.name_resolver.resolve(&self.getting);

        //Look for a structure of that name
        if self.operation {
            if let Some(found) = locked.operations.get(name) {
                return Poll::Ready(Ok(found.get(0).unwrap().clone()));
            }
        } else {
            if let Some(found) = locked.functions.types.get(name).cloned() {
                self.finished = Some(found.clone());

                return Poll::Ready(Ok(found));
            }
        }

        if locked.async_manager.finished && !locked.functions.parsing.contains(&name) {
            println!("Failed to find {}!", name);
            locked.functions.wakers.values().for_each(|wakers| for waker in wakers {
                waker.wake_by_ref();
            });
            return Poll::Ready(Err(self.error.clone()));
        }

        //Add a waker for that type
        if let Some(vectors) = locked.functions.wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            locked.functions.wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        return Poll::Pending;
    }
}

impl Future for AsyncTypesGetter<Struct> {
    type Output = Result<Arc<Struct>, ParsingError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        let name = self.name_resolver.resolve(&self.getting);

        //Look for a structure of that name
        if let Some(found) = locked.structures.types.get(name).cloned() {
            self.finished = Some(found.clone());

            return Poll::Ready(Ok(found));
        }

        if locked.async_manager.finished && !locked.structures.parsing.contains(&name) {
            locked.structures.wakers.values().for_each(|wakers| for waker in wakers {
                waker.wake_by_ref();
            });
            return Poll::Ready(Err(self.error.clone()));
        }

        //Add a waker for that type
        if let Some(vectors) = locked.structures.wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            locked.structures.wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        return Poll::Pending;
    }
}

#[derive(Clone, Debug)]
pub enum UnparsedType {
    Basic(String),
    Generic(Box<UnparsedType>, Vec<UnparsedType>)
}

pub trait NameResolver: Send + Sync {
    fn resolve<'a>(&'a self, name: &'a String) -> &'a String;

    fn generic(&self, name: &String) -> Option<Vec<UnparsedType>>;

    fn boxed_clone(&self) -> Box<dyn NameResolver>;
}