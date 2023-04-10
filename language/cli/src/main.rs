use std::env;
use tokio::runtime::Builder;
use crate::arguments::Arguments;

pub mod arguments;

fn main() {
    let arguments = Arguments::from_arguments(env::args());
    let runner = Builder::new_current_thread().thread_name("main").build().unwrap();
    runner.block_on(run(&arguments));
}

async fn run(arguments: &Arguments) {
    match runner::runner::run::<(), u64>(&arguments.runner_settings).await {
        Err(errors) => {
            for error in errors {
                println!("{}", error);
            }
        },
        Ok(result) => {
            println!("Exit code: {}", unsafe { result(()) });
        }
    }
}