use std::fmt::Display;
use std::mem;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::Sender;
use anyhow::Error;
use checker::output::TypesChecker;
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::{Main, Syntax};
use crate::get_compiler;
use data::RunnerSettings;
use no_deadlocks::Mutex;
use tokio::runtime::Handle;

#[no_mangle]
pub fn run_extern(handle: Handle, target: &'static str, settings: &RunnerSettings)
                         -> Result<Option<Main<()>>, Vec<ParsingError>> {
    return unsafe { mem::transmute(handle.block_on(run::<u64>(target, settings))) };
}

pub async fn run<T: Display + Send + 'static>(target: &'static str, settings: &RunnerSettings)
                                    -> Result<Option<Box<T>>, Vec<ParsingError>> {
    let syntax = Syntax::new(Box::new(
        TypesChecker::new(settings.cpu_runtime.handle().clone(), settings.include_references())));
    let syntax = Arc::new(Mutex::new(syntax));

    let (sender, receiver) = mpsc::channel();

    settings.cpu_runtime.spawn(start(target, settings.compiler.clone(), sender, syntax.clone()));

    //Parse source, getting handles and building into the unresolved syntax.
    let mut handles = Vec::new();
    for source_set in &settings.sources {
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

    if !errors.is_empty() {
        for error in errors {
            println!("Error: {}", error)
        }
        return Err(Vec::new());
    }

    return receiver.recv().unwrap();
}

pub async fn start<T: Display>(target: &str, compiler: String, sender: Sender<Result<Option<Box<T>>, Vec<ParsingError>>>, syntax: Arc<Mutex<Syntax>>) {
    let code_compiler;
    {
        let locked = syntax.lock().unwrap();
        code_compiler = get_compiler(locked.compiling.clone(),
                                     locked.strut_compiling.clone(), compiler);
    }
    let returning = code_compiler.compile(target, &syntax);
    sender.send(returning).unwrap();
}