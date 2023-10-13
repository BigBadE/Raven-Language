use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::hash::Hash;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc};
use std::task::{Context, Poll, Waker};
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;

use crate::{ParsingError, TopElement};
use crate::function::display_parenless;
use crate::syntax::Syntax;

/// A future that asynchronously gets a type from its respective AsyncGetter.
/// Will never deadlock because types are added to the AsyncGetter before being finalized.
pub struct AsyncTypesGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,

    pub error: ParsingError,
    pub getting: String,
    pub name_resolver: Box<dyn NameResolver>,
    pub not_trait: bool,
    pub finished: Option<Arc<T>>
}

/// A future that asynchronously gets a type's finalized type from its respective AsyncGetter.
/// Will never deadlock as long finalized types don't depend on it.
pub struct AsyncDataGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub getting: Arc<T>
}

impl<T: TopElement> AsyncTypesGetter<T> {
    /// Helper method to try a get a type with the given prefix, and adding a waker if not.
    fn get_types(&mut self, locked: &mut Syntax, prefix: String, waker: Waker, not_trait: bool)
                 -> Option<Result<Arc<T>, ParsingError>> {
        // Add the prefix to the name, if any.
        let name = if prefix.is_empty() {
            self.getting.clone()
        } else if prefix.ends_with(&self.getting) {
            prefix
        } else {
            prefix + "::" + &*self.getting.clone()
        };

        let getting = T::get_manager(locked);
        //Look for a structure of that name
        if let Some(found) = getting.types.get(&name).cloned() {
            if !not_trait || !found.is_trait() {
                self.finished = Some(found.clone());
                return Some(Ok(found));
            }
        }

        //Add a waker for that type
        if let Some(vectors) = getting.wakers.get_mut(&name) {
            vectors.push(waker);
        } else {
            getting.wakers.insert(name, vec!(waker));
        }

        return None;
    }
}

impl<T: TopElement> AsyncTypesGetter<T> {
    pub fn new(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String,
                    name_resolver: Box<dyn NameResolver>, not_trait: bool) -> Self {
        return Self {
            syntax,
            error,
            getting,
            name_resolver,
            finished: None,
            not_trait
        };
    }
}

impl<T: TopElement> Future for AsyncTypesGetter<T> {
    type Output = Result<Arc<T>, ParsingError>;

    /// Gets the top element from the structure with the given name, using the given imports.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If we found it already, return it.
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let not_trait = self.not_trait;
        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        // Check if a structure directly referenced with that name exists.
        if let Some(output) = self.get_types(&mut locked,
                                             String::new(), cx.waker().clone(), not_trait) {
            return Poll::Ready(output);
        }

        // Check each import if the structure is in those files.
        for import in self.name_resolver.imports().clone() {
            if let Some(output) = self.get_types(&mut locked,
                                                 import, cx.waker().clone(), not_trait) {
                return Poll::Ready(output);
            }
        }

        // If the async manager is finished, return an error.
        if locked.async_manager.finished {
            println!("Error for {} from {:?}: {}", self.getting, T::get_manager(locked.deref_mut())
                .types.keys(), self.error);
            return Poll::Ready(Err(self.error.clone()));
        }

        // Parsing isn't finished, so this sleeps.
        return Poll::Pending;
    }
}

impl<T: TopElement> AsyncDataGetter<T> {
    pub fn new(syntax: Arc<Mutex<Syntax>>, getting: Arc<T>) -> Self {
        return AsyncDataGetter {
            syntax,
            getting
        }
    }
}

impl<T> Future for AsyncDataGetter<T> where T: TopElement + Hash + Eq + Debug {
    type Output = Arc<T::Finalized>;

    /// Look for the finalized element given the data.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        let manager = T::get_manager(locked.deref_mut());
        // The finalized element exists, return
        if let Some(output) = manager.data.get(&self.getting) {
            return Poll::Ready(output.clone());
        }

        // The finalized element doesn't exist, sleep.
        if let Some(wakers) = manager.wakers.get_mut(self.getting.name()) {
            wakers.push(cx.waker().clone());
        } else {
            manager.wakers.insert(self.getting.name().clone(), vec!(cx.waker().clone()));
        }

        // This never panics because as long as the data exists, every element will be finalized.
        return Poll::Pending;
    }
}

// A type that hasn't been parsed yet, used for types that need to be clonable before they're finalized.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnparsedType {
    Basic(String),
    Generic(Box<UnparsedType>, Vec<UnparsedType>),
}

impl Display for UnparsedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            UnparsedType::Basic(name) => write!(f, "{}", name),
            UnparsedType::Generic(base, bounds) =>
                write!(f, "{}<{}>", base, display_parenless(bounds, " + "))
        };
    }
}

/// A name resolver gives the async utils generic access to data used by later compilation steps.
pub trait NameResolver: Send + Sync {
    fn imports(&self) -> &Vec<String>;

    fn generic(&self, name: &String) -> Option<Vec<UnparsedType>>;

    fn generics(&self) -> &HashMap<String, Vec<UnparsedType>>;

    /// Clones the name resolver in a box, because it's a trait it can't be directly cloned.
    fn boxed_clone(&self) -> Box<dyn NameResolver>;
}