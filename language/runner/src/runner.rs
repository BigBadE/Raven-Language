use std::fs;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::Sender;
use anyhow::Error;
use checker::output::TypesChecker;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::{Output, Syntax};
use crate::{get_compiler, RunnerSettings};

pub async fn run(settings: &RunnerSettings)
    -> Result<Option<Output>, Vec<ParsingError>> {
    let syntax = Syntax::new(Box::new(TypesChecker::new(settings.cpu_runtime.handle().clone())));
    let syntax = Arc::new(Mutex::new(syntax));

    let (sender, receiver) = mpsc::channel();

    settings.cpu_runtime.spawn(start(settings.compiler.clone(), sender, syntax.clone()));

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
        println!("Error detected, this likely poisoned the mutexes. Please report any non-poison errors");
        for error in errors {
            println!("{}", error)
        }
        return Err(Vec::new());
    }

    return receiver.recv().unwrap();
}

pub async fn start(compiler: String, sender: Sender<Result<Option<Output>, Vec<ParsingError>>>, syntax: Arc<Mutex<Syntax>>) {
    let code_compiler;
    {
        let locked = syntax.lock().unwrap();
        code_compiler = get_compiler(locked.compiling.clone(),
                                locked.strut_compiling.clone(), compiler);
    }
    sender.send(code_compiler.compile(&syntax)).unwrap();
}