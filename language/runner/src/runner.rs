use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time;

use checker::output::TypesChecker;
use data::{Arguments, CompilerArguments};
use parser::parse;
use syntax::async_util::HandleWrapper;
use syntax::syntax::Syntax;
use syntax::ParsingError;

use crate::{get_compiler, JoinWaiter};

/// Runs Raven to completion with the given arguments
pub async fn run<T: Send + 'static>(settings: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    let handle = Arc::new(Mutex::new(HandleWrapper::new(settings.cpu_runtime.handle().clone())));
    let mut syntax = Syntax::new(Box::new(TypesChecker::new(handle.clone(), settings.runner_settings.include_references())));
    syntax.async_manager.target.clone_from(&settings.runner_settings.compiler_arguments.target);

    let syntax = Arc::new(Mutex::new(syntax));

    let (sender, mut receiver) = mpsc::channel(1);
    let (go_sender, go_receiver) = mpsc::channel(1);

    // Starts the compiler in anticipation of parsing
    settings.cpu_runtime.spawn(start(
        settings.runner_settings.compiler_arguments.clone(),
        sender,
        go_receiver,
        syntax.clone(),
    ));

    let mut handles = Vec::default();
    // Parses source, getting handles and building into the unresolved syntax.
    for source_set in &settings.runner_settings.sources {
        for file in source_set.get_files() {
            if !file.path().ends_with("rv") {
                continue;
            }

            handles.push(
                settings
                    .io_runtime
                    .as_ref()
                    .map(|inner| inner.handle().clone())
                    .unwrap_or_else(|| settings.cpu_runtime.handle().clone())
                    .spawn(parse(syntax.clone(), handle.clone(), source_set.relative(&*file).clone(), file)),
            );
        }
    }

    let mut errors = Vec::default();
    //Join any compilers errors
    for handle in handles {
        match handle.await {
            Err(error) => errors.push(Error::new(error)),
            Ok(_) => {}
        }
    }

    if !errors.is_empty() {
        for error in errors {
            println!("Error: {}", error);
        }
        panic!("Error detected!");
    }

    syntax.lock().unwrap().finish();

    match time::timeout(Duration::from_secs(5), JoinWaiter { handle: handle.clone() }).await {
        Ok(_) => {}
        Err(_) => {
            for (name, _) in &handle.lock().unwrap().names {
                println!("Infinite loop for {}", name);
            }
            panic!();
        }
    }

    let errors = syntax.lock().unwrap().errors.clone();
    return if errors.is_empty() {
        go_sender.send(()).await.unwrap();
        Ok(receiver.recv().await.unwrap())
    } else {
        Err(errors)
    };
}

/// Runs the compiler, waiting for the receiver before running the main function then sending the result on the sender.
pub async fn start<T>(
    compiler_arguments: CompilerArguments,
    sender: Sender<Option<T>>,
    receiver: Receiver<()>,
    syntax: Arc<Mutex<Syntax>>,
) {
    let code_compiler;
    {
        let locked = syntax.lock().unwrap();
        code_compiler = get_compiler(locked.compiling.clone(), locked.strut_compiling.clone(), compiler_arguments);
    }

    let _ = sender.send(code_compiler.compile(receiver, &syntax).await).await;
}
