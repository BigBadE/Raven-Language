use std::collections::HashMap;
use std::future::Future;
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use data::ParsingError;

use crate::async_util::NameResolver;
use crate::code::FinalizedEffects;
use crate::function::FunctionData;
use crate::syntax::Syntax;
use crate::types::FinalizedTypes;
use crate::TopElement;

/// The async manager, just stores basic information about the current parsing state.
#[derive(Default)]
pub struct GetterManager {
    //If parsing non-impls is finished
    pub finished: bool,
    //How many impls are still being parsed, which is done async and not tied to finished
    pub parsing_impls: u32,
    //Impl waiters, which are woken whenever an impl finishes parsing.
    pub impl_waiters: Vec<Waker>,

    pub target: String,
    pub target_waker: Option<Waker>,
}

pub struct ImplWaiter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub return_type: FinalizedTypes,
    pub data: FinalizedTypes,
    pub error: ParsingError,
}

impl Future for ImplWaiter {
    type Output = Result<Vec<Arc<FunctionData>>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        return match locked.get_implementation_methods(&self.return_type, &self.data) {
            Some(found) => Poll::Ready(Ok(found)),
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

pub struct TraitImplWaiter<F> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub resolver: Box<dyn NameResolver>,
    pub method: String,
    pub return_type: FinalizedTypes,
    pub checker: F,
    pub error: ParsingError,
}

impl<T: Future<Output = Result<FinalizedEffects, ParsingError>>, F: Fn(Arc<FunctionData>) -> T>
    Future for TraitImplWaiter<F>
{
    type Output = Result<FinalizedEffects, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return match pin!(find_trait_implementation(
            &self.syntax,
            &*self.resolver,
            &self.method,
            &self.return_type
        ))
        .poll(cx)
        {
            Poll::Ready(inner) => match inner {
                Ok(inner) => match inner {
                    Some(found) => {
                        for trying in found {
                            match pin!((self.checker)(trying)).poll(cx) {
                                Poll::Ready(found) => match found {
                                    Ok(found) => return Poll::Ready(Ok(found)),
                                    Err(_) => {}
                                },
                                Poll::Pending => return Poll::Pending,
                            }
                        }
                        self.syntax
                            .lock()
                            .unwrap()
                            .async_manager
                            .impl_waiters
                            .push(cx.waker().clone());
                        Poll::Pending
                    }
                    None => {
                        if self.syntax.lock().unwrap().finished_impls() {
                            Poll::Ready(Err(self.error.clone()))
                        } else {
                            self.syntax
                                .lock()
                                .unwrap()
                                .async_manager
                                .impl_waiters
                                .push(cx.waker().clone());
                            Poll::Pending
                        }
                    }
                },
                Err(error) => return Poll::Ready(Err(error)),
            },
            Poll::Pending => Poll::Pending,
        };
    }
}

pub async fn find_trait_implementation(
    syntax: &Arc<Mutex<Syntax>>,
    resolver: &dyn NameResolver,
    method: &String,
    return_type: &FinalizedTypes,
) -> Result<Option<Vec<Arc<FunctionData>>>, ParsingError> {
    let mut output = Vec::default();

    for import in resolver.imports() {
        if let Ok(value) = Syntax::get_struct(
            syntax.clone(),
            ParsingError::empty(),
            import.split("::").last().unwrap().to_string(),
            resolver.boxed_clone(),
            vec![],
        )
        .await
        {
            let value = value.finalize(syntax.clone()).await;
            if let Some(value) = syntax
                .lock()
                .unwrap()
                .get_implementation_methods(&return_type, &value)
            {
                for temp in &value {
                    if &temp.name.split("::").last().unwrap() == method {
                        output.push(temp.clone());
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

pub struct TypeWaiter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub current: FinalizedTypes,
    pub other: FinalizedTypes,
}

impl Future for TypeWaiter {
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
/// top element manager, holds the top elements and the wakers requiring those elements.
/// Wakers are used to allow tasks to wait for an element to be parsed and added
pub struct TopElementManager<T>
where
    T: TopElement,
{
    //Types and their data, added immediately after parsing
    pub types: HashMap<String, Arc<T>>,
    //A list of data sorted by the data's ID. Guaranteed to be in ID order.
    pub sorted: Vec<Arc<T>>,
    //Data sorted by its finalized type, which contains the finalized code. Added after finalization.
    pub data: HashMap<Arc<T>, Arc<T::Finalized>>,
    //Wakers waiting on a type to be added to the types hashmap, waked after the type is added to types
    pub wakers: HashMap<String, Vec<Waker>>,
}

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
    //Creates the getter with a list of sorted types already, used for internal types declared in the compiler
    pub fn with_sorted(sorted: Vec<Arc<T>>) -> Self {
        return Self {
            types: HashMap::default(),
            sorted,
            data: HashMap::default(),
            wakers: HashMap::default(),
        };
    }
}
