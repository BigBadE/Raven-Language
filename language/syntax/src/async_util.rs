use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::function::Function;
use crate::ParsingError;
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
    type Output = Result<Arc<Struct>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.structures.get(name) {
            return Poll::Ready(Ok(found.clone()));
        }
        if locked.finished {
            return Poll::Ready(Err(ParsingError::new((0, 0), (0, 0),
                                                     format!("Failed to find {}", name))));
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
    type Output = Result<Arc<Function>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.static_functions.get(name) {
            return Poll::Ready(Ok(found.clone()));
        }

        if locked.finished {
            return Poll::Ready(Err(ParsingError::new((0, 0), (0, 0),
                                                     format!("Failed to find function {}", name))));
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
    fn resolve<'a>(&self, name: &'a String) -> &'a String;
}

pub struct EmptyNameResolver {}

impl NameResolver for EmptyNameResolver {
    fn resolve<'a>(&self, name: &'a String) -> &'a String {
        name
    }
}