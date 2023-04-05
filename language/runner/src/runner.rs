use std::fs;
use std::sync::{Arc, Mutex};
use anyhow::Error;
use checker::output::TypesCompiler;
use compilers::compiling::CompiledProject;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::RunnerSettings;

pub async fn run(settings: &RunnerSettings) -> Result<Box<dyn CompiledProject>, Vec<ParsingError>> {
    let compiler = settings.get_compiler();

    let syntax = Arc::new(Mutex::new(Syntax::new(Box::new(
        TypesCompiler::new(settings.cpu_runtime.handle().clone(), compiler.clone())))));

    //Parse source, getting handles and building into the unresolved syntax.
    let mut handles = Vec::new();
    for source_set in &settings.sources {
        for file in source_set.get_files() {
            handles.push(settings.io_runtime.spawn(parse(syntax.clone(), source_set.relative(&file).clone(),
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
            Ok(result) => match result {
                Err(error) => {
                    errors.push(error);
                }
                Ok(()) => {}
            }
        }
    }

    //Set the syntax to finished because parsing is done.
    //This starts deadlock detection
    syntax.lock().unwrap().finish();

    if !errors.is_empty() {
        println!("Error detected, this likely poisoned the mutexes. Report the non-poison errors.");
        for error in errors {
            println!("{}", error)
        }
        return Err(Vec::new());
    }
    
    return compiler.compile().await;
}