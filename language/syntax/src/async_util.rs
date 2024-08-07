use indexmap::IndexMap;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::hash::Hash;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use tokio::runtime::Handle;
use tokio::task::{AbortHandle, JoinHandle};

use data::tokens::Span;

use crate::errors::{ErrorSource, ParsingMessage};
use crate::program::function::display_parenless;
use crate::program::syntax::Syntax;
use crate::program::types::FinalizedTypes;
use crate::{FinishedStructImplementor, ParsingError, TopElement};

/// A future that asynchronously gets a type from its respective AsyncGetter.
/// Will never deadlock because types are added to the AsyncGetter before being finalized.
pub struct AsyncTypesGetter<T: TopElement> {
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// The error to return on fail
    pub error: ParsingError,
    /// The type being gotten
    pub getting: String,
    /// The name resolver, used to get imports
    pub name_resolver: Box<dyn NameResolver>,
    /// Whether to ignore traits
    pub not_trait: bool,
    /// The finished value, if this has finished already
    pub finished: Option<Arc<T>>,
}

/// A future that asynchronously gets a type's finalized type from its respective AsyncGetter.
/// Will never deadlock as long finalized types don't depend on it.
pub struct AsyncDataGetter<T: TopElement> {
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// Type to get
    pub getting: Arc<T>,
}

impl<T: TopElement> AsyncTypesGetter<T> {
    /// Helper method to try a get a type with the given prefix, and adding a waker if not.
    fn get_types(
        &mut self,
        locked: &mut Syntax,
        prefix: String,
        waker: Waker,
        not_trait: bool,
    ) -> Option<Result<Arc<T>, ParsingError>> {
        // Add the prefix to the name, if any.
        let name = if prefix.is_empty() {
            self.getting.clone()
        } else if prefix.ends_with(&self.getting) {
            prefix
        } else {
            prefix + "::" + &*self.getting.clone()
        };

        let getting = T::get_manager(locked);
        //Look for a program of that name
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
            getting.wakers.insert(name, vec![waker]);
        }

        return None;
    }

    /// Cleans up extra implementation waiters made by this type, to preserve memory
    fn clean_up(&self, syntax: &mut Syntax, imports: &Vec<String>) {
        // Can't clean till parsing is over
        if !syntax.async_manager.finished {
            return;
        }

        let manager = T::get_manager(syntax);
        if let Some(found) = manager.wakers.remove(&self.getting) {
            for waker in found {
                waker.wake();
            }
        }

        for import in imports {
            let import =
                if import.ends_with(&self.getting) { import.clone() } else { format!("{}::{}", import, self.getting) };
            if let Some(found) = manager.wakers.remove(&import) {
                for waker in found {
                    waker.wake();
                }
            }
        }
    }
}

impl<T: TopElement> AsyncTypesGetter<T> {
    /// Creates a new types getter
    pub fn new(
        syntax: Arc<Mutex<Syntax>>,
        error: Span,
        getting: String,
        name_resolver: Box<dyn NameResolver>,
        not_trait: bool,
    ) -> Self {
        return Self {
            syntax,
            error: error.make_error(ParsingMessage::FailedToFind(getting.clone())),
            getting,
            name_resolver,
            finished: None,
            not_trait,
        };
    }
}

impl<T: TopElement> Future for AsyncTypesGetter<T> {
    type Output = Result<Arc<T>, ParsingError>;

    /// Gets the top element from the program with the given name, using the given imports.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If we found it already, return it.
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let not_trait = self.not_trait;
        let locked = self.syntax.clone();
        let mut locked = locked.lock();

        // Check if an element directly referenced with that name exists.
        if let Some(output) = self.get_types(&mut locked, String::default(), cx.waker().clone(), not_trait) {
            self.clean_up(&mut locked, self.name_resolver.imports());
            return Poll::Ready(output);
        }

        // Check each import if the element is in those files.
        for import in self.name_resolver.imports().clone() {
            if let Some(output) = self.get_types(&mut locked, import.clone(), cx.waker().clone(), not_trait) {
                self.clean_up(&mut locked, self.name_resolver.imports());
                return Poll::Ready(output);
            }
        }

        // If the async manager is finished, return an error.
        if locked.async_manager.finished {
            return Poll::Ready(Err(self.error.clone()));
        }

        // Parsing isn't finished, so this sleeps.
        return Poll::Pending;
    }
}

impl<T: TopElement> AsyncDataGetter<T> {
    /// Creates a new data getter
    pub fn new(syntax: Arc<Mutex<Syntax>>, getting: Arc<T>) -> Self {
        return AsyncDataGetter { syntax, getting };
    }
}

impl<T> Future for AsyncDataGetter<T>
where
    T: TopElement + Hash + Eq + Debug,
{
    type Output = Arc<T::Finalized>;

    /// Look for the finalized element given the data.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let locked = self.syntax.clone();
        let mut locked = locked.lock();

        let manager = T::get_manager(locked.deref_mut());

        // The finalized element exists, return
        if let Some(output) = manager.data.get(&self.getting) {
            return Poll::Ready(output.clone());
        }

        // The finalized element doesn't exist, sleep.
        manager.wakers.entry(self.getting.name().clone()).or_insert(vec![]).push(cx.waker().clone());

        // This never panics because as long as the data exists, every element will be finalized.
        return Poll::Pending;
    }
}

/// Asynchronously gets the implementation of a structure
pub struct AsyncStructImplGetter {
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// Type to get
    pub getting: FinalizedTypes,
}

impl AsyncStructImplGetter {
    /// Creates a new async struct impl getter
    pub fn new(syntax: Arc<Mutex<Syntax>>, getting: FinalizedTypes) -> Self {
        return Self { syntax, getting };
    }
}

impl Future for AsyncStructImplGetter {
    type Output = Vec<Arc<FinishedStructImplementor>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock();
        if !locked.finished_impls() {
            locked.async_manager.impl_waiters.push(cx.waker().clone());
            return Poll::Pending;
        }

        let mut output = vec![];
        for (types, impls) in &locked.struct_implementations {
            if self.getting.of_type_sync(&types, None).0 {
                for found_impl in impls {
                    output.push(found_impl.clone());
                }
            }
        }

        return Poll::Ready(output);
    }
}

/// A type that hasn't been parsed yet, used for types that need to be clonable before they're finalized.
#[derive(Clone, Debug)]
pub enum UnparsedType {
    /// Basic types are just a string
    Basic(Span, String),
    /// A generic-bound type, with a base type and bounds
    Generic(Box<UnparsedType>, Vec<UnparsedType>),
}

impl UnparsedType {
    pub fn get_span(&self) -> Span {
        return match self {
            UnparsedType::Basic(span, _) => span.clone(),
            UnparsedType::Generic(base, bounds) => {
                let mut output = base.get_span().clone();
                if !bounds.is_empty() {
                    output.extend_span(bounds.last().as_ref().unwrap().get_span().end + 1);
                }
                output
            }
        };
    }
}

impl Display for UnparsedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            UnparsedType::Basic(_, name) => write!(f, "{}", name),
            UnparsedType::Generic(base, bounds) => {
                write!(f, "{}<{}>", base, display_parenless(bounds, " + "))
            }
        };
    }
}

/// A name resolver gives the async utils generic access to data used by later compilation steps.
pub trait NameResolver: Send + Sync {
    /// This function's imports
    fn imports(&self) -> &Vec<String>;

    /// Finds the generic given the name
    fn generic(&self, name: &String) -> Option<Vec<UnparsedType>>;

    /// All of this function's generics
    fn generics(&self) -> &IndexMap<String, Vec<UnparsedType>>;

    /// All of this function's generics, mutably
    fn generics_mut(&mut self) -> &mut IndexMap<String, Vec<UnparsedType>>;

    /// Clones the name resolver in a box, because it's a trait it can't be directly cloned.
    fn boxed_clone(&self) -> Box<dyn NameResolver>;
}

/// An empty vec
static EMPTY: Vec<String> = vec![];

/// An empty name resolver, for after finalization
pub struct EmptyNameResolver {}

impl NameResolver for EmptyNameResolver {
    fn imports(&self) -> &Vec<String> {
        return &EMPTY;
    }

    fn generic(&self, _name: &String) -> Option<Vec<UnparsedType>> {
        panic!("Should not be called after finalizing!")
    }

    fn generics(&self) -> &IndexMap<String, Vec<UnparsedType>> {
        panic!("Should not be called after finalizing!")
    }

    fn generics_mut(&mut self) -> &mut IndexMap<String, Vec<UnparsedType>> {
        panic!("Should not be called after finalizing!")
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(EmptyNameResolver {});
    }
}

/// Wraps around a Handle, allowing the program to wait for all spawned tasks to finish
pub struct HandleWrapper {
    /// The inner handle
    handle: Handle,
    /// Tasks to join to finish
    pub joining: Vec<JoinHandle<Result<(), ParsingError>>>,
    /// The names of running tasks and a handle to abort them
    pub names: HashMap<String, AbortHandle>,
    /// A waker to wake when finished with a task
    pub waker: Option<Waker>,
}

impl HandleWrapper {
    /// Creates a new handle wrapper
    pub fn new(handle: Handle) -> HandleWrapper {
        return HandleWrapper { handle, joining: vec![], names: HashMap::default(), waker: None };
    }
    /// Spawns a task and adds it to the joining vec
    pub fn spawn<F: Future<Output = Result<(), ParsingError>> + Send + 'static>(&mut self, name: String, future: F) {
        let handle = self.handle.spawn(future);
        self.names.insert(name, handle.abort_handle());

        self.joining.push(handle);
    }

    /// Tells the wrapper that a task finished, the waker will remove the handle from the handles vec
    pub fn finish_task(&mut self, name: &String) {
        self.names.remove(name);
        if let Some(found) = &self.waker {
            found.wake_by_ref();
        }
    }
}
