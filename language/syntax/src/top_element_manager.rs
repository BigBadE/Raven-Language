use std::collections::HashMap;
use std::future::Future;
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use data::tokens::Span;
use data::ParsingError;

use crate::async_util::NameResolver;
use crate::code::FinalizedEffects;
use crate::function::FunctionData;
use crate::syntax::Syntax;
use crate::types::FinalizedTypes;
use crate::{FinishedTraitImplementor, TopElement};

/// The async manager, just stores basic information about the current parsing state.
#[derive(Default)]
pub struct GetterManager {
    /// If parsing non-impls is finished
    pub finished: bool,
    /// How many impls are still being parsed, which is done async and not tied to finished
    pub parsing_impls: u32,
    /// Impl waiters, which are woken whenever an impl finishes parsing.
    pub impl_waiters: Vec<Waker>,
    /// The target method to compile
    pub target: String,
    /// Waker to wake when the target method is found
    pub target_waker: Option<Waker>,
}

/// Waits for an implementation of the type
pub struct ImplWaiter {
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// The type being checked
    pub base_type: FinalizedTypes,
    /// The base type
    pub trait_type: FinalizedTypes,
    /// Error if the type isn't found
    pub error: ParsingError,
}

impl Future for ImplWaiter {
    type Output = Result<Vec<Arc<FunctionData>>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        return match locked.get_implementation_methods(&self.base_type, &self.trait_type) {
            Some(found) => Poll::Ready(Ok(found.into_iter().map(|(_, inner)| inner).flatten().collect())),
            None => {
                if locked.finished_impls() {
                    Poll::Ready(Err(self.error.clone()))
                } else {
                    locked.async_manager.impl_waiters.push(cx.waker().clone());
                    Poll::Pending
                }
            }
        };
    }
}

/// Waits for an implementation of the trait matching the constraints
pub struct TraitImplWaiter<F> {
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// Name resolver and its imports
    pub resolver: Box<dyn NameResolver>,
    /// Name of the method
    pub method: String,
    /// The type being checked
    pub return_type: FinalizedTypes,
    /// A future that checks if the function is valid
    pub checker: F,
    /// Error to return if none is found
    pub error: ParsingError,
}

impl<
        T: Future<Output = Result<FinalizedEffects, ParsingError>>,
        F: Fn(Arc<FinishedTraitImplementor>, Arc<FunctionData>) -> T,
    > Future for TraitImplWaiter<F>
{
    type Output = Result<FinalizedEffects, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return match pin!(find_trait_implementation(&self.syntax, &*self.resolver, &self.method, &self.return_type)).poll(cx)
        {
            Poll::Ready(inner) => match inner {
                Ok(inner) => {
                    match inner {
                        Some(found) => {
                            for (types, trying) in found {
                                for func in trying {
                                    match pin!((self.checker)(types.clone(), func)).poll(cx) {
                                        Poll::Ready(found) => match found {
                                            Ok(found) => return Poll::Ready(Ok(found)),
                                            Err(_) => {}
                                        },
                                        Poll::Pending => {}
                                    }
                                }
                            }
                        }
                        None => {}
                    }
                    if self.syntax.lock().unwrap().finished_impls() {
                        Poll::Ready(Err(self.error.clone()))
                    } else {
                        self.syntax.lock().unwrap().async_manager.impl_waiters.push(cx.waker().clone());
                        Poll::Pending
                    }
                }
                Err(error) => return Poll::Ready(Err(error)),
            },
            Poll::Pending => {
                let mut locked = self.syntax.lock().unwrap();
                locked.async_manager.impl_waiters.push(cx.waker().clone());
                Poll::Pending
            }
        };
    }
}

/// Finds all the implementations of the type
pub async fn find_trait_implementation(
    syntax: &Arc<Mutex<Syntax>>,
    resolver: &dyn NameResolver,
    method: &String,
    return_type: &FinalizedTypes,
) -> Result<Option<Vec<(Arc<FinishedTraitImplementor>, Vec<Arc<FunctionData>>)>>, ParsingError> {
    let mut output = Vec::default();

    for import in resolver.imports() {
        if let Ok(value) = Syntax::get_struct(
            syntax.clone(),
            ParsingError::new(
                Span::default(),
                "You shouldn't see this! Report this please! Location: Find trait implementation",
            ),
            import.split("::").last().unwrap().to_string(),
            resolver.boxed_clone(),
            vec![],
        )
        .await
        {
            let value = value.finalize(syntax.clone()).await;
            if let Some(value) = syntax.lock().unwrap().get_implementation_methods(&return_type, &value) {
                for (types, functions) in &value {
                    for temp in functions {
                        let mut current = vec![];
                        if &temp.name.split("::").last().unwrap() == method {
                            current.push(temp.clone());
                        }
                        if !current.is_empty() {
                            output.push((types.clone(), current))
                        }
                    }
                }
            }
        }
    }
    if output.is_empty() {
        return Ok(None);
    } else {
        return Ok(Some(output));
    }
}

/// Tries to solve if a type implements another type
pub struct TypeImplementsTypeWaiter {
    /// The program
    pub syntax: Arc<Mutex<Syntax>>,
    /// Base type
    pub current: FinalizedTypes,
    /// Other type
    pub other: FinalizedTypes,
}

impl Future for TypeImplementsTypeWaiter {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        // Only check for implementations if being compared against a trait.
        // Wait for the implementation to finish.
        if locked.solve(&self.current, &self.other) {
            return Poll::Ready(true);
        }

        if !locked.finished_impls() {
            locked.async_manager.impl_waiters.push(cx.waker().clone());
            return Poll::Pending;
        }

        // Now all impls are parsed so solve is correct.
        return Poll::Ready(false);
    }
}

/// Holds the top elements and the wakers requiring those elements.
/// Wakers are used to allow tasks to wait for an element to be parsed and added
pub struct TopElementManager<T>
where
    T: TopElement,
{
    /// Types and their data, added immediately after parsing
    pub types: HashMap<String, Arc<T>>,
    /// A list of data sorted by the data's ID. Guaranteed to be in ID order.
    pub sorted: Vec<Arc<T>>,
    /// Data sorted by its finalized type, which contains the finalized code. Added after finalization.
    pub data: HashMap<Arc<T>, Arc<T::Finalized>>,
    /// Wakers waiting on a type to be added to the types hashmap, waked after the type is added to types
    pub wakers: HashMap<String, Vec<Waker>>,
}

impl<T: TopElement> TopElementManager<T> {
    /// Wakes up all sleepers for the given name
    fn wake(&mut self, name: &String) {
        if let Some(wakers) = self.wakers.remove(name) {
            for waker in wakers {
                waker.wake();
            }
        }
    }

    /// Sets the correct ID on the data type
    pub fn set_id(&mut self, data: &mut T) {
        data.set_id(
            self.sorted.iter().position(|found| found.name() == data.name()).unwrap_or_else(|| self.sorted.len()) as u64
        );
    }

    /// Adds the type to the list of types
    pub fn add_type(&mut self, data: Arc<T>) {
        self.wake(data.name());
        if !self.sorted.contains(&data) {
            self.sorted.push(data.clone());
        }
        self.types.insert(data.name().clone(), data);
    }

    /// Adds the finalized data to the list of types.
    pub fn add_data(&mut self, types: Arc<T>, data: Arc<T::Finalized>) {
        self.wake(types.name());
        self.data.insert(types, data);
    }
}

/// Rust's derive breaks this for some reason so it's manually implemented
impl<T: TopElement> Default for TopElementManager<T> {
    fn default() -> Self {
        return Self {
            types: HashMap::default(),
            sorted: Vec::default(),
            data: HashMap::default(),
            wakers: HashMap::default(),
        };
    }
}

impl<T> TopElementManager<T>
where
    T: TopElement,
{
    /// Creates the getter with a list of sorted types already, used for internal types declared in the compiler
    pub fn with_sorted(sorted: Vec<Arc<T>>) -> Self {
        return Self { types: HashMap::default(), sorted, data: HashMap::default(), wakers: HashMap::default() };
    }
}
