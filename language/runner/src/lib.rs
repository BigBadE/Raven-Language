use compiler_llvm::LLVMCompiler;
use dashmap::DashMap;
use data::CompilerArguments;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};
use syntax::async_util::HandleWrapper;
use syntax::program::function::FinalizedFunction;
use syntax::program::r#struct::FinalizedStruct;
use syntax::program::syntax::Compiler;

/// The main Raven runner
pub mod runner;

/// Gets the compiler given the name and the compiling Arcs (so they can be passed to the compiler)
pub fn get_compiler<T>(
    compiling: Arc<DashMap<String, Arc<FinalizedFunction>>>,
    struct_compiling: Arc<DashMap<String, Arc<FinalizedStruct>>>,
    arguments: CompilerArguments,
) -> Box<dyn Compiler<T> + Send + Sync> {
    return Box::new(match arguments.compiler.to_lowercase().as_str() {
        "llvm" => LLVMCompiler::new(compiling, struct_compiling, arguments),
        _ => panic!("Unknown compilers {}", arguments.compiler),
    });
}

/// A future used to wait for the handle to finish
pub struct JoinWaiter {
    /// The handle to wait on
    handle: Arc<Mutex<HandleWrapper>>,
}

impl Future for JoinWaiter {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut locked = self.handle.lock().unwrap();

        let mut removing = Vec::default();

        let mut i = 0;
        for mut handle in &mut locked.joining {
            if handle.is_finished() {
                removing.push(i);
                i += 1;
            } else {
                match Pin::new(&mut handle).poll(cx) {
                    Poll::Ready(inner) => match inner {
                        Ok(_) => {
                            removing.push(i);
                            i += 1;
                        }
                        Err(error) => {
                            panic!("{}", error);
                        }
                    },
                    Poll::Pending => {}
                }
            }
        }

        removing.reverse();
        for found in removing {
            locked.joining.remove(found);
        }
        return if locked.joining.is_empty() {
            Poll::Ready(false)
        } else {
            locked.waker = Some(cx.waker().clone());
            Poll::Pending
        };
    }
}
