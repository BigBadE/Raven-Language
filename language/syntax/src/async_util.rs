use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use lazy_static::lazy_static;

use crate::function::Function;
use crate::r#struct::Struct;
use crate::syntax::Syntax;

pub struct StructureGetter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>
}

impl StructureGetter {
    pub fn new(syntax: Arc<Mutex<Syntax>>, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            getting,
            name_resolver
        };
    }
}

impl Future for StructureGetter {
    type Output = Arc<Struct>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.structures.get(name) {
            return Poll::Ready(found.clone());
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
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>
}

impl FunctionGetter {
    pub fn new(syntax: Arc<Mutex<Syntax>>, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            getting,
            name_resolver
        };
    }
}

impl Future for FunctionGetter {
    type Output = Arc<Function>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.static_functions.get(name) {
            return Poll::Ready(found.clone());
        }
        if let Some(vectors) = locked.function_wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            locked.function_wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        return Poll::Pending;
    }
}

pub trait NameResolver {
    fn resolve(&self, name: &String) -> &String;
}

pub struct EmptyNameResolver {}

impl NameResolver for EmptyNameResolver {
    fn resolve(&self, name: &String) -> &String {
        name
    }
}