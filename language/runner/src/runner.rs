use std::fs;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::Sender;
use anyhow::Error;
use checker::output::TypesChecker;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::{get_compiler, RunnerSettings};

use no_deadlocks::Mutex;

pub async fn run<T: Send + 'static>(target: &'static str, settings: &RunnerSettings)
    -> Result<Option<T>, Vec<ParsingError>> {
    let syntax = Syntax::new(Box::new(
        TypesChecker::new(settings.cpu_runtime.handle().clone(), settings.include_references())));
    let syntax = Arc::new(Mutex::new(syntax));

    let (sender, receiver) = mpsc::channel();

    settings.cpu_runtime.spawn(start(target, settings.compiler.clone(), sender, syntax.clone()));

    //Parse source, getting handles and building into the unresolved syntax.
    let mut handles = Vec::new();
    for source_set in &settings.sources {
        for file in source_set.get_files() {
            if !file.as_path().to_str().unwrap().ends_with("rv") {
                continue;
            }
            handles.push(
                settings.io_runtime.spawn(parse(syntax.clone(), settings.io_runtime.handle().clone(),
                                                source_set.relative(&file).clone(),
                                                fs::read_to_string(file.clone()).expect(
                                                    &format!("Failed to read source file: {}", file.to_str().unwrap())))));
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

    if !errors.is_empty() {
        for error in errors {
            println!("Error: {}", error)
        }
        return Err(Vec::new());
    }

    return receiver.recv().unwrap();
}

pub async fn start<T>(target: &str, compiler: String, sender: Sender<Result<Option<T>, Vec<ParsingError>>>, syntax: Arc<Mutex<Syntax>>) {
    let code_compiler;
    {
        let locked = syntax.lock().unwrap();
        code_compiler = get_compiler(locked.compiling.clone(),
                                locked.strut_compiling.clone(), compiler);
    }
    let returning = code_compiler.compile(target, &syntax);
    sender.send(returning).unwrap();
}