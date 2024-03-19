use std::env;

use data::{Arguments, CompilerArguments, RunnerSettings};
use magpie_lib::project::RavenProject;
use magpie_lib::{build_project, InnerSourceSet, MAGPIE};
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
                target: "build::project".to_string(),
                compiler: "llvm".to_string(),
                temp_folder: env::current_dir().unwrap().join("target"),
            },
        },
    );

    println!("Setting up build...");
    let project = match build_project::<RavenProject>(
        &mut arguments,
        &mut vec![Box::new(FileSourceSet { root: build_path }), Box::new(InnerSourceSet { set: &MAGPIE })],
        true,
    ) {
        Ok((_, found)) => match found {
            Some(found) => RavenProject::from(found),
            None => {
                println!("No project method in build file!");
                return;
            }
        },
        Err(()) => return,
    };

    arguments.runner_settings.compiler_arguments.target = "main::main".to_string();

    let source = env::current_dir().unwrap().join("src");

    if !source.exists() {
        panic!("Source folder (src) not found!");
    }

    println!("Building and running {}...", project.name);
    match build_project::<()>(&mut arguments, &mut vec![Box::new(FileSourceSet { root: source })], true) {
        _ => {}
    }
}
