use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::function::Function;
use crate::ParsingError;
use crate::syntax::Syntax;
use crate::types::Types;

pub struct StructureGetter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub error: ParsingError,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>
}

impl StructureGetter {
    pub fn new(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            name_resolver
        };
    }
}

impl Future for StructureGetter {
    type Output = Result<Types, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        if let Some(found) = self.name_resolver.generic(&self.getting) {
            return Poll::Ready(Ok(found));
        }
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.structures.get(name) {
            return Poll::Ready(Ok(Types::Struct(found.clone())));
        }
        if locked.finished {
            return Poll::Ready(Err(self.error.clone()));
        }
        if let Some(vectors) = locked.structure_wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            locked.structure_wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        return Poll::Pending;
    }
}

pub struct FunctionGetter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub error: ParsingError,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>
}

impl FunctionGetter {
    pub fn new(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            name_resolver
        };
    }
}

impl Future for FunctionGetter {
    type Output = Result<Arc<Function>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.functions.get(name) {
            return Poll::Ready(Ok(found.clone()));
        }

        if locked.finished {
            return Poll::Ready(Err(self.error.clone()));
        }
        if let Some(vectors) = locked.function_wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            locked.function_wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        return Poll::Pending;
    }
}

pub trait NameResolver: Send + Sync {
    fn resolve<'a>(&'a self, name: &'a String) -> &'a String;
    
    fn generic(&self, name: &String) -> Option<Types>;
}