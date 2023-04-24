use std::fs;
use std::sync::{Arc, Mutex};
use anyhow::Error;
use checker::output::TypesChecker;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::RunnerSettings;

pub async fn run(settings: &RunnerSettings)
    -> Result<Option<i64>, Vec<ParsingError>> {
    let compiler = settings.get_compiler();

    let syntax = Arc::new(Mutex::new(Syntax::new(
        Box::new(TypesChecker::new(settings.cpu_runtime.handle().clone())))));

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

    if !errors.is_empty() {
        println!("Error detected, this likely poisoned the mutexes. Please report any non-poison errors");
        for error in errors {
            println!("{}", error)
        }
        return Err(Vec::new());
    }

    return compiler.compile(&syntax);
}