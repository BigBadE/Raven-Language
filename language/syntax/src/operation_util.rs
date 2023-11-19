use crate::r#struct::StructData;
use crate::syntax::Syntax;
use crate::ParsingError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};

/// An asynchronous getter for operations given the operation.
pub struct OperationGetter {
    pub syntax: Arc<Mutex<Syntax>>,
    pub operation: Vec<String>,
    pub error: ParsingError,
}

impl Future for OperationGetter {
    type Output = Result<Arc<StructData>, ParsingError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let locked = self.syntax.clone();
        let mut locked = locked.lock().unwrap();

        for operation in &self.operation {
            if let Some(output) = locked.operations.get(operation) {
                return Poll::Ready(Ok(output.clone()));
            } else if let Some(output) = locked
                .operations
                .get(&operation.replace("{}", "{+}").to_string())
            {
                return Poll::Ready(Ok(output.clone()));
            }
        }

        if locked.async_manager.finished {
            return Poll::Ready(Err(self.error.clone()));
        }

        for operation in &self.operation {
            if let Some(found) = locked.operation_wakers.get_mut(operation) {
                found.push(cx.waker().clone());
            } else {
                locked
                    .operation_wakers
                    .insert(operation.clone(), vec![cx.waker().clone()]);
            }
        }

        return Poll::Pending;
    }
}
