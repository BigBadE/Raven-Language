use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use crate::{ParsingError, TopElement};
use crate::async_getters::AsyncGetter;
use crate::function::{display_parenless, Function};
use crate::r#struct::Struct;
use crate::syntax::Syntax;

pub(crate) struct AsyncTypesGetter<T: TopElement> {
    pub syntax: Arc<Mutex<Syntax>>,
    pub error: ParsingError,
    pub getting: String,
    pub operation: bool,
    pub name_resolver: Box<dyn NameResolver>,
    pub finished: Option<Arc<T>>,
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

impl AsyncTypesGetter<Function> {
    pub fn new_func(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, operation: bool, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            operation,
            name_resolver,
            finished: None,
        };
    }
}

impl AsyncTypesGetter<Struct> {
    pub fn new_struct(syntax: Arc<Mutex<Syntax>>, error: ParsingError, getting: String, name_resolver: Box<dyn NameResolver>) -> Self {
        return Self {
            syntax,
            error,
            getting,
            operation: false,
            name_resolver,
            finished: None,
        };
    }
}

impl Future for AsyncTypesGetter<Function> {
    type Output = Result<Arc<Function>, ParsingError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(finished) = &self.finished {
            return Poll::Ready(Ok(finished.clone()));
        }

        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        //Look for a structure of that name
        if self.operation {
            if let Some(found) = locked.operations.get(&self.getting) {
                return Poll::Ready(Ok(found.get(0).unwrap().clone()));
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
            if locked.structures.parsing.contains(&self.getting) {
                return Poll::Pending;
            }
            for imports in self.name_resolver.imports() {
                if locked.structures.parsing.contains(&format!("{}::{}", imports, self.getting)) {
                    return Poll::Pending;
                }
            }
            locked.functions.wakers.values().for_each(|wakers| for waker in wakers {
                waker.wake_by_ref();
            });
            return Poll::Ready(Err(self.error.clone()));
        }

        return Poll::Pending;
    }
}

impl Future for AsyncTypesGetter<Struct> {
    type Output = Result<Arc<Struct>, ParsingError>;

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
            if locked.structures.parsing.contains(&self.getting) {
                return Poll::Pending;
            }
            for imports in self.name_resolver.imports() {
                if locked.structures.parsing.contains(&format!("{}::{}", imports, self.getting)) {
                    return Poll::Pending;
                }
            }

            locked.structures.wakers.values().for_each(|wakers| for waker in wakers {
                waker.wake_by_ref();
            });

            return Poll::Ready(Err(self.error.clone()));
        }

        return Poll::Pending;
    }
}

#[derive(Clone, Debug)]
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