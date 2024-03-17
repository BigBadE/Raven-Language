use std::sync::Arc;

use parking_lot::Mutex;

use data::{Arguments, RavenExtern, SourceSet};
use parser::FileSourceSet;
use syntax::program::syntax::Syntax;

use crate::build_project;
use crate::project::RavenProject;

pub fn build_file<T: RavenExtern + 'static>(
    arguments: &mut Arguments,
    sources: &mut Vec<Box<dyn SourceSet>>,
    compile: bool,
    project: bool,
) -> Result<(Arc<Mutex<Syntax>>, Option<T>), ()> {
    if !project {
        return build_project::<T>(arguments, sources, compile);
    }

    let build_sources = arguments.runner_settings.compiler_arguments.temp_folder.parent().unwrap().join("build.rv");
    arguments.runner_settings.compiler_arguments.target = "build::project".to_string();
    let project = build_project::<RavenProject>(arguments, &mut vec![Box::new(FileSourceSet { root: build_sources })], true)
        .unwrap()
        .1
        .unwrap();

    // TODO use project for dependencies

    arguments.runner_settings.compiler_arguments.target = "main::main".to_string();
    return build_project(arguments, sources, compile);
}
