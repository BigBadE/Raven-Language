use std::env;

use data::{Arguments, CompilerArguments, RunnerSettings};
use magpie_lib::{build_project, build_project_file};
use parser::FileSourceSet;

mod test;

/// Finds the Raven project/file and runs it
fn main() {
    let args = env::args().collect::<Vec<_>>();

    if args.len() == 2 {
    } else if args.len() > 2 {
        panic!("Unknown extra arguments! {:?}", args);
    }

    let build_path = env::current_dir().unwrap().join("build.rv");

    if !build_path.exists() {
        println!("Build file not found!");
        return;
    }

    let mut arguments = Arguments::build_args(
        false,
        RunnerSettings {
            sources: vec![],
            compiler_arguments: CompilerArguments {
                target: String::default(),
                compiler: "llvm".to_string(),
                temp_folder: env::current_dir().unwrap().join("target"),
            },
        },
    );

    println!("Setting up build...");
    let _project = match build_project_file(&mut arguments, build_path) {
        Ok(project) => project,
        Err(error) => {
            println!("{}", error);
            return;
        }
    };
    arguments.runner_settings.compiler_arguments.target = "main::main".to_string();

    let source = env::current_dir().unwrap().join("src");

    if !source.exists() {
        panic!("Source folder (src) not found!");
    }

    println!("Building and running");
    match build_project::<()>(&mut arguments, &mut vec![Box::new(FileSourceSet { root: source })], true) {
        _ => {}
    }
}
