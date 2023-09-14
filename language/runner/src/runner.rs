use std::{alloc, ptr};
use std::alloc::Layout;
use std::ffi::{c_char, c_int, CString};
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::mpsc::Sender;

use anyhow::Error;
use no_deadlocks::Mutex;
use tokio::runtime::Builder;

use checker::output::TypesChecker;
use data::{Arguments, RunnerSettings};
use parser::parse;
use syntax::ParsingError;
use syntax::syntax::Syntax;

use crate::get_compiler;

// Technically these types aren't C FFI-able, but Rust can understand them.
#[no_mangle]
pub extern fn run_extern(target: String, settings: RunnerSettings) -> Result<Option<AtomicPtr<()>>, Vec<ParsingError>> {
    let project= Builder::new_current_thread().thread_name("Raven Main").build().unwrap()
        .block_on(run::<AtomicPtr<RawRavenProject>>(target,
                                                    &Arguments::build_args(false, settings)))?;
    unsafe {
        let project = ptr::read(project.unwrap().load(Ordering::Relaxed));
        println!("Id: {}", project.type_id);
        println!("Name: {}", CString::from_raw(project.name.load(Ordering::Relaxed)).to_str().unwrap().to_string());
    }

    return Ok(None);
}

#[derive(Debug)]
#[repr(C, align(8))]
pub struct RawRavenProject {
    type_id: c_int,
    pub name: AtomicPtr<c_char>,
}

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

    if !errors.is_empty() {
        for error in errors {
            println!("Error: {}", error)
        }
        return Err(Vec::new());
    }

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
    sender.send(returning).unwrap();
}