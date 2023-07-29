use std::env;
use tokio::runtime::Builder;
use crate::arguments::Arguments;

pub mod arguments;

fn main() {
    let arguments = Arguments::from_arguments(env::args());
    println!("Compiling from source roots: {:?}", arguments.runner_settings.sources);
    let runner = Builder::new_current_thread().thread_name("main").build().unwrap();
    runner.block_on(run(&arguments));
}

async fn run(arguments: &Arguments) {
    let compiler = runner::runner::run::<i64>(&arguments.runner_settings).await;
    match compiler {
        Err(errors) => {
            for error in errors {
                println!("{}", error);
            }
        },
        Ok(result) => {
            match result {
                Some(result) => println!("Exit code: {}", result),
                None => println!("No main found!")
            }
        }
    }
}