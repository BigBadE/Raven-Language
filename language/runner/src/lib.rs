use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use compiler_llvm::LLVMCompiler;
use data::CompilerArguments;
use syntax::async_util::HandleWrapper;
use syntax::function::FinalizedFunction;
use syntax::r#struct::FinalizedStruct;
use syntax::syntax::Compiler;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;

pub mod runner;

pub fn get_compiler<T>(compiling: Arc<RwLock<HashMap<String, Arc<FinalizedFunction>>>>,
                       struct_compiling: Arc<RwLock<HashMap<String, Arc<FinalizedStruct>>>>,
                       arguments: CompilerArguments) -> Box<dyn Compiler<T> + Send + Sync> {
    return Box::new(match arguments.compiler.to_lowercase().as_str() {
        "llvm" => LLVMCompiler::new(compiling, struct_compiling, arguments),
        _ => panic!("Unknown compilers {}", arguments.compiler)
    });
}

pub struct JoinWaiter {
    handle: Arc<Mutex<HandleWrapper>>
}

impl Future for JoinWaiter {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let next = self.handle.lock().unwrap().joining.pop();

        return if let Some(mut found) = next {
            match Pin::new(&mut found).poll(cx) {
                Poll::Ready(inner) => if inner.is_err() {
                    return Poll::Ready(true)
                }
                Poll::Pending => self.handle.lock().unwrap().joining.insert(0, found)
            }
            Poll::Pending
        } else {
            Poll::Ready(false)
        }
    }
}