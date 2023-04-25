use std::future::Future;
use std::ops::DerefMut;
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
        syntax.lock().unwrap().locked += 1;
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
        locked.locked -= 1;

        println!("Woke for {}: {:?}", self.getting, locked.structures.keys());
        //Look for a generic of that name
        if let Some(found) = self.name_resolver.generic(&self.getting) {
            return Poll::Ready(Ok(found));
        }
        //Look for a structure of that name
        let name = self.name_resolver.resolve(&self.getting);
        if let Some(found) = locked.structures.get(name) {
            println!("Found!");
            return Poll::Ready(Ok(Types::Struct(found.clone())));
        }

        println!("Failed!");
        check_wake(locked.deref_mut());

        if locked.locked >= locked.remaining {
            println!("Error");
            return Poll::Ready(Err(self.error.clone()));
        }

        //Add a waker for that type
        if let Some(vectors) = locked.structure_wakers.get_mut(name) {
            vectors.push(cx.waker().clone());
        } else {
            locked.structure_wakers.insert(self.getting.clone(), vec!(cx.waker().clone()));
        }
        println!("Pending");
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
        syntax.lock().unwrap().locked += 1;
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
        locked.locked -= 1;

        let name = self.name_resolver.resolve(&self.getting);

        if let Some(found) = locked.functions.get(name) {
            return Poll::Ready(Ok(found.clone()));
        }

        check_wake(locked.deref_mut());

        if locked.locked >= locked.remaining {
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
    
    fn boxed_clone(&self) -> Box<dyn NameResolver>;
}

fn check_wake(locked: &mut Syntax) {
    locked.locked += 1;

    //If this is the last running task, fail it all.
    if locked.locked == locked.remaining {
        locked.structure_wakers.values().for_each(|wakers| for waker in wakers {
            waker.wake_by_ref();
        });
        locked.function_wakers.values().for_each(|wakers| for waker in wakers {
            waker.wake_by_ref();
        });
    }

}