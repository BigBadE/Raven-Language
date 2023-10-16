use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use crate::ParsingError;
use crate::r#struct::StructData;
use crate::syntax::Syntax;

/// An asynchronous getter for operations given the operation.
pub struct OperationGetter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub operation: String,
    pub error: ParsingError
}

impl Future for OperationGetter {
    type Output = Result<Arc<StructData>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        if let Some(output) = locked.operations.get(&self.operation) {
            return Poll::Ready(Ok(output.clone()));
        } else if let Some(output) = locked.operations.get(&self.operation.replace("{}", "{+}").to_string()) {
            return Poll::Ready(Ok(output.clone()));
        }

        if locked.async_manager.finished {
            return Poll::Ready(Err(self.error.clone()));
        }

        if let Some(found) = locked.operation_wakers.get_mut(&self.operation) {
            found.push(cx.waker().clone());
        } else {
            locked.operation_wakers.insert(self.operation.clone(), vec!(cx.waker().clone()));
        }

        return Poll::Pending;
    }
}