use std::env;
use tokio::main;
use runner::{RunnerSettings, SourceSet};
use syntax::ParsingError;
use crate::arguments::Arguments;
use include_dir::{Dir, include_dir};

pub mod arguments;

static LIBRARY: Dir = include_dir!("lib/core");
static CORE: Dir = include_dir!("tools/magpie/lib");

#[main]
async fn main() {
    let base_arguments = Arguments::from_arguments(env::args());
    let build_path = env::current_dir().unwrap().join("build.rv");

    let arguments = Arguments {
        runner_settings: RunnerSettings {
            io_runtime: base_arguments.runner_settings.io_runtime,
            cpu_runtime: base_arguments.runner_settings.cpu_runtime,
            sources: vec!(SourceSet {
                root: build_path,
            }, SourceSet {
                root: LIBRARY.path().clone().to_path_buf()
            }, SourceSet {
                root: CORE.path().clone().to_path_buf()
            }, ),
            debug: false,
            compiler: "llvm".to_string(),
        },
    };
    let value = run::<i64>(&arguments).await;
}

async fn run<T: Send + 'static>(arguments: &Arguments) -> Result<Option<T>, Vec<ParsingError>> {
    return runner::runner::run::<T>("build::project", &arguments.runner_settings).await;
}