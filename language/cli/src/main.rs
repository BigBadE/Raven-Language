use std::env;
use tokio::runtime::Builder;
use data::Arguments;
use crate::arguments::from_arguments;

pub mod arguments;

fn main() {
    let arguments = from_arguments(env::args());
    println!("Compiling from source roots: {:?}", arguments.runner_settings.sources);
    let runner = Builder::new_current_thread().thread_name("main").build().unwrap();
    runner.block_on(run(&arguments));
}

async fn run(arguments: &Arguments) {
    let compiler = runner::runner::run::<u64>(
        "main::main".to_string(), &arguments).await;
    match compiler {
        Err(errors) => {
            println!("Errors detected:");
            for error in errors {
                println!("{}", error);
            }
        },
        Ok(result) => {
            println!("Done!");
            match result {
                Some(result) => println!("Exit code: {}", result),
                None => println!("No main found!")
            }
        }
    }
}