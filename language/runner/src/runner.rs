use std::fs;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::Sender;
use anyhow::Error;
use checker::output::TypesChecker;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::{get_compiler, RunnerSettings};

pub async fn run(settings: &RunnerSettings)
    -> Result<Option<i64>, Vec<ParsingError>> {
    let syntax = Syntax::new(
        Box::new(TypesChecker::new(settings.cpu_runtime.handle().clone())));
    let syntax = Arc::new(Mutex::new(syntax));
    syntax.lock().unwrap().process_manager.init(syntax.clone());

    let (sender, receiver) = mpsc::channel();

    settings.cpu_runtime.spawn(start(sender, settings.compiler.clone(), syntax.clone()));

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

pub async fn start(sender: Sender<Result<Option<i64>, Vec<ParsingError>>>, compiler: String, syntax: Arc<Mutex<Syntax>>) {
    let compiler = get_compiler(compiler);
    sender.send(compiler.compile(&syntax)).unwrap();
}