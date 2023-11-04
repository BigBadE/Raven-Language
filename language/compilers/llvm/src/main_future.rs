use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use syntax::syntax::Syntax;

use std::sync::Mutex;
use std::task::{Context, Poll};
use syntax::function::FinalizedFunction;

pub struct MainFuture {
    pub syntax: Arc<Mutex<Syntax>>,
}

impl Future for MainFuture {
    type Output = Arc<FinalizedFunction>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.syntax.lock().unwrap();
        let compiling = locked.compiling.clone();
        let compiling = compiling.read().unwrap();
        return if let Some(found) = compiling.get(&locked.async_manager.target) {
            Poll::Ready(found.clone())
        } else {
            locked.async_manager.target_waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}