use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use crate::function::Function;
use crate::r#struct::Struct;
use crate::syntax::Syntax;
use crate::types::Types;

pub struct StructureGetter<T> where T: Types {
    pub syntax: Arc<Mutex<Syntax<T>>>,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>
}

impl<T> StructureGetter<T> where T: Types {
    pub fn new(syntax: Arc<Mutex<Syntax<T>>>, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            getting,
            name_resolver
        };
    }
}

impl<T> Future for StructureGetter<T> where T: Types {
    type Output = Arc<Struct<T>>;

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

pub struct FunctionGetter<T> where T: Types {
    pub syntax: Arc<Mutex<Syntax<T>>>,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>
}

impl<T> FunctionGetter<T> where T: Types {
    pub fn new(syntax: Arc<Mutex<Syntax<T>>>, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            getting,
            name_resolver
        };
    }
}

impl<T> Future for FunctionGetter<T> where T: Types {
    type Output = Arc<Function<T>>;

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