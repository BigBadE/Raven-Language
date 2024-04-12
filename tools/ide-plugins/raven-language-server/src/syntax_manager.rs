use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use data::{Arguments, CompilerArguments, RunnerSettings, SourceSet};
use magpie_lib::{build_project, build_project_file};
use parser::FileSourceSet;
use syntax::program::syntax::Syntax;

#[derive(Default)]
pub struct SyntaxManager {
    pub parents: HashMap<PathBuf, Arc<Mutex<Syntax>>>,
}

impl SyntaxManager {
    pub fn get_syntax(&mut self, file: PathBuf) -> Arc<Mutex<Syntax>> {
        if let Some(found) = self.parents.get(&file) {
            return found.clone();
        }
        return self.update_syntax(file);
    }

    pub fn update_syntax(&mut self, file: PathBuf) -> Arc<Mutex<Syntax>> {
        let mut arguments = Arguments::build_args(
            false,
            RunnerSettings {
                sources: vec![],
                compiler_arguments: CompilerArguments { compiler: "llvm".to_string(), ..Default::default() },
            },
        );

        let mut directory = Self::get_project(&mut arguments, &file);

        self.parents.insert(file.clone(), build_project::<()>(&mut arguments, &mut directory, false).unwrap().0);
        return self.parents.get(&file).unwrap().clone();
    }

    fn get_project(arguments: &mut Arguments, file: &PathBuf) -> Vec<Box<dyn SourceSet>> {
        let mut directory = file.parent();
        while let Some(dir) = directory {
            if dir.join("build.rv").exists() {
                break;
            }
            directory = dir.parent();
        }
        return if let Some(directory) = directory {
            let _unused = build_project_file(arguments, directory.to_path_buf());
            vec![Box::new(FileSourceSet { root: file.clone() })]
        } else {
            vec![Box::new(FileSourceSet { root: file.clone() })]
        };
    }
}
