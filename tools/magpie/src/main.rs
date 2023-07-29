use std::{env, fs};
use tokio::main;
use tokio::runtime::Builder;
use runner::{RunnerSettings, SourceSet};
use syntax::ParsingError;
use crate::arguments::Arguments;

pub mod arguments;

#[main]
async fn main() {
    let base_arguments = Arguments::from_arguments(env::args());
    let build_path = env::current_dir().unwrap().join("build.rv");

    let arguments = Arguments {
        runner_settings: RunnerSettings {
            io_runtime: base_arguments.runner_settings.io_runtime.clone(),
            cpu_runtime: base_arguments.runner_settings.cpu_runtime.clone(),
            sources: vec!(SourceSet {
                root: build_path,
            }),
            debug: false,
            compiler: "llvm".to_string(),
        },
    };
    let value = run::<i64>(&arguments).await;
}

async fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    return runner::runner::run::<T>(&arguments.runner_settings).await;
}