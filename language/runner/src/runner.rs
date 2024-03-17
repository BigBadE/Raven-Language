use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time;

use checker::output::TypesChecker;
use data::{Arguments, CompilerArguments};
use parser::parse;
use syntax::async_util::HandleWrapper;
use syntax::errors::ParsingError;
use syntax::program::syntax::Syntax;

use crate::{get_compiler, JoinWaiter};

pub fn create_syntax(settings: &Arguments) -> Arc<Mutex<Syntax>> {
    let handle = Arc::new(Mutex::new(HandleWrapper::new(settings.cpu_runtime.handle().clone())));
    let mut syntax = Syntax::new(Box::new(TypesChecker::new(handle.clone(), settings.runner_settings.include_references())));
    syntax.async_manager.target.clone_from(&settings.runner_settings.compiler_arguments.target);
    return Arc::new(Mutex::new(syntax));
}

pub async fn build(syntax: Arc<Mutex<Syntax>>, settings: &Arguments) -> Result<(), Vec<ParsingError>> {
    let handle = syntax.lock().process_manager.handle().clone();

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
    //Join any parsing errors
    for handle in handles {
        match time::timeout(Duration::from_secs(60), handle).await {
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

    syntax.lock().finish();

    let mut errors = vec![];
    let waiter = JoinWaiter { handle: handle.clone() };
    match time::timeout(Duration::from_secs(60), waiter).await {
        Ok(error) => match error {
            Err(error) => {
                errors.push(error);
            }
            _ => {}
        },
        Err(_) => {
            println!("Detected infinite loops:");
            for (name, _) in &handle.lock().names {
                println!("Infinite loop for {}", name);
            }
            let length = handle.lock().joining.len();
            panic!("Failed to parse with {} ({}) infinite loops", length, handle.lock().names.len());
        }
    }

    errors.append(&mut syntax.lock().errors);
    return if errors.is_empty() { Ok(()) } else { Err(errors) };
}

/// Runs Raven to completion with the given arguments
pub async fn run<T: Send + 'static>(
    syntax: Arc<Mutex<Syntax>>,
    settings: &Arguments,
) -> Result<Option<T>, Vec<ParsingError>> {
    let (sender, mut receiver) = mpsc::channel(1);
    let (go_sender, go_receiver) = mpsc::channel(1);

    // Starts the compiler in anticipation of parsing
    settings.cpu_runtime.spawn(start(
        settings.runner_settings.compiler_arguments.clone(),
        sender,
        go_receiver,
        syntax.clone(),
    ));

    build(syntax.clone(), settings).await?;

    go_sender.send(()).await.unwrap();
    return Ok(receiver.recv().await.unwrap());
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
        let locked = syntax.lock();
        code_compiler = get_compiler(locked.compiling.clone(), locked.strut_compiling.clone(), compiler_arguments);
    }

    let _ = sender.send(code_compiler.compile(receiver, &syntax).await).await;
}
