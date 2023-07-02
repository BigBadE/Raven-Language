use std::fmt::{Display, Formatter};
use std::future::Future;
use std::hash::Hash;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use crate::{ParsingError, TopElement};
use crate::async_getters::AsyncGetter;
use crate::function::{display_parenless, FunctionData};
use crate::r#struct::StructData;
use crate::syntax::Syntax;

pub(crate) struct AsyncTypesGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub error: ParsingError,
    pub getting: String,
    pub operation: bool,
    pub name_resolver: Box<dyn NameResolver>,
    pub finished: Option<Arc<T>>
}

pub struct AsyncDataGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub getting: Arc<T>
}

impl<T: TopElement> AsyncDataGetter<T> {
    pub fn new(syntax: Arc<Mutex<Syntax>>, getting: Arc<T>) -> Self {
        return AsyncDataGetter {
            syntax,
            getting
        }
    }
}

impl<T: TopElement> AsyncTypesGetter<T> {
    fn get_types(&mut self, getting: &mut AsyncGetter<T>, name: String, waker: Waker) -> Option<Result<Arc<T>, ParsingError>> {
        let name = if name.is_empty() {
            self.getting.clone()
        } else {
            name + "::" + &*self.getting.clone()
        };

        //Look for a structure of that name
        if let Some(found) = getting.types.get(&name).cloned() {
            self.finished = Some(found.clone());

            return Some(Ok(found));
        }

        //Add a waker for that type
        if let Some(vectors) = getting.wakers.get_mut(&name) {
            vectors.push(waker);
        } else {
            getting.wakers.insert(self.getting.clone(), vec!(waker));
        }
        return None;
    }
}

impl AsyncTypesGetter<FunctionData> {
    pub fn new_func(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, operation: bool, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            operation,
            name_resolver,
            finished: None
        };
    }
}

impl AsyncTypesGetter<StructData> {
    pub fn new_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            operation: false,
            name_resolver,
            finished: None
        };
    }
}

impl Future for AsyncTypesGetter<FunctionData> {
    type Output = Result<Arc<FunctionData>, ParsingError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        //Look for a structure of that name
        if self.operation {
            if let Some(found) = locked.operations.get(&self.getting) {
                let data: &Arc<FunctionData> = found.get(0).unwrap();
                return Poll::Ready(Ok(data.clone()));
            }
        }

        if let Some(output) = self.get_types(&mut locked.functions,
                                             String::new(), cx.waker().clone()) {
            return Poll::Ready(output);
        }

        for import in self.name_resolver.imports().clone() {
            if let Some(output) = self.get_types(&mut locked.functions,
                                                 import, cx.waker().clone()) {
                return Poll::Ready(output);
            }
        }

        if locked.async_manager.finished {
            locked.functions.wakers.values().for_each(|wakers| for waker in wakers {
                waker.wake_by_ref();
            });
            return Poll::Ready(Err(self.error.clone()));
        }

        return Poll::Pending;
    }
}

impl Future for AsyncTypesGetter<StructData> {
    type Output = Result<Arc<StructData>, ParsingError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        if let Some(output) = self.get_types(&mut locked.structures,
                                             String::new(), cx.waker().clone()) {
            return Poll::Ready(output);
        }

        for import in self.name_resolver.imports().clone() {
            if let Some(output) = self.get_types(&mut locked.structures,
                                                 import, cx.waker().clone()) {
                return Poll::Ready(output);
            }
        }

        if locked.async_manager.finished {
            locked.structures.wakers.values().for_each(|wakers| for waker in wakers {
                waker.wake_by_ref();
            });

            return Poll::Ready(Err(self.error.clone()));
        }

        return Poll::Pending;
    }
}

impl<T> Future for AsyncDataGetter<T> where T: TopElement + Hash + Eq {
    type Output = Arc<T::Finalized>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        if let Some(output) = T::get_manager(locked.deref_mut()).data.get(&self.getting) {
            return Poll::Ready(output.clone());
        }

        return Poll::Pending;
    }
}

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

pub trait NameResolver: Send + Sync {
    fn imports(&self) -> &Vec<String>;

    fn generic(&self, name: &String) -> Option<Vec<UnparsedType>>;

    fn boxed_clone(&self) -> Box<dyn NameResolver>;
}