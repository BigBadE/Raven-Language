use std::sync::{Arc, mpsc};
use std::sync::mpsc::Sender;

use anyhow::Error;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;

use checker::output::TypesChecker;
use data::Arguments;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::Syntax;

use crate::get_compiler;

pub async fn run<T: Send + 'static>(target: String, settings: &Arguments)
                                    -> Result<Option<T>, Vec<ParsingError>> {
    let syntax = Syntax::new(Box::new(
        TypesChecker::new(settings.cpu_runtime.handle().clone(), settings.runner_settings.include_references())));
    let syntax = Arc::new(Mutex::new(syntax));

    let (sender, receiver) = mpsc::channel();

    settings.cpu_runtime.spawn(start(target, settings.runner_settings.compiler.clone(), sender, syntax.clone()));

    //Parse source, getting handles and building into the unresolved syntax.
    let mut handles = Vec::new();
    for source_set in &settings.runner_settings.sources {
        for file in source_set.get_files() {
            if !file.path().ends_with("rv") {
                continue;
            }

            handles.push(
                settings.io_runtime.spawn(parse(syntax.clone(), settings.io_runtime.handle().clone(),
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

    syntax.lock().unwrap().finish();

    return receiver.recv().unwrap();
}

pub async fn start<T>(target: String, compiler: String, sender: Sender<Result<Option<T>, Vec<ParsingError>>>, syntax: Arc<Mutex<Syntax>>) {
    let code_compiler;
    {
        let locked = syntax.lock().unwrap();
        code_compiler = get_compiler(locked.compiling.clone(),
                                     locked.strut_compiling.clone(), compiler);
    }

    let returning = code_compiler.compile(target, &syntax);
    let errors = &syntax.lock().unwrap().errors;
    if errors.is_empty() {
        sender.send(Ok(returning)).unwrap();
    } else {
        sender.send(Err(errors.clone())).unwrap();
    }
}