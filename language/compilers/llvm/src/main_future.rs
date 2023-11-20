use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use syntax::syntax::Syntax;

use std::sync::Mutex;
use std::task::{Context, Poll};
use syntax::function::FinalizedFunction;

/// Future for finding the main function
pub struct MainFuture {
    // Program syntax
    pub syntax: Arc<Mutex<Syntax>>,
}

impl Future for MainFuture {
    type Output = Arc<FinalizedFunction>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        locked.async_manager.target_waker = Some(cx.waker().clone());
        return match locked.compiling.get(&locked.async_manager.target) {
            Some(found) => Poll::Ready(found.clone()),
            None => Poll::Pending,
        };
    }
}
