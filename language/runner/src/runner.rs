use std::sync::Arc;

use anyhow::Error;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

use checker::output::TypesChecker;
use data::Arguments;
use parser::parse;
use syntax::async_util::HandleWrapper;
use syntax::ParsingError;
use syntax::syntax::Syntax;

use crate::get_compiler;

pub async fn run<T: Send + 'static>(target: String, settings: &Arguments)
                                    -> Result<Option<T>, Vec<ParsingError>> {
    let mut syntax = Syntax::new(Box::new(
        TypesChecker::new(settings.cpu_runtime.handle().clone(), settings.runner_settings.include_references())));
    syntax.async_manager.target = target.clone();

    let syntax = Arc::new(Mutex::new(syntax));

    let (sender, mut receiver) = mpsc::channel(1);
    let (go_sender, go_receiver) = mpsc::channel(1);

    settings.cpu_runtime.spawn(start(target, settings.runner_settings.compiler.clone(), sender, go_receiver, syntax.clone()));

    //Parse source, getting handles and building into the unresolved syntax.
    let handle = Arc::new(Mutex::new(HandleWrapper { handle: settings.cpu_runtime.handle().clone(), joining: vec!() }));
    let mut handles = Vec::new();
    for source_set in &settings.runner_settings.sources {
        for file in source_set.get_files() {
            if !file.path().ends_with("rv") {
                continue;
            }

            handles.push(
                settings.io_runtime.spawn(parse(syntax.clone(), handle.clone(),
                                                source_set.relative(&file).clone(),
                                                file.read())));
        }
    }

    let mut errors = Vec::new();
    //Join any compilers errors
    for handle in handles {
        match handle.await {
            Err(error) => {
                errors.push(Error::new(error))
            }
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

    let mut failed = false;
    while let Some(found) = handle.lock().unwrap().joining.pop() {
        match found.await {
            Err(error) => {
                failed = true;
                println!("Error: {}", error);
            },
            _ => {}
        }
    }
    if failed {
        panic!("Error detected!");
    }

    let errors = syntax.lock().unwrap().errors.clone();
    if errors.is_empty() {
        go_sender.send(()).await.unwrap();
        return Ok(receiver.recv().await.unwrap());
    } else {
        return Err(errors);
    }
}

pub async fn start<T>(target: String, compiler: String, sender: Sender<Option<T>>, receiver: Receiver<()>, syntax: Arc<Mutex<Syntax>>) {
    let code_compiler;
    {
        let locked = syntax.lock().unwrap();
        code_compiler = get_compiler(locked.compiling.clone(),
                                     locked.strut_compiling.clone(), compiler);
    }

    sender.send(code_compiler.compile(target, receiver, &syntax).await).await.unwrap();
}