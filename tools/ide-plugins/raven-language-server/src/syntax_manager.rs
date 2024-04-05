use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use data::{Arguments, CompilerArguments, RunnerSettings, SourceSet};
use magpie_lib::build_project;
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
        let mut directory = Self::get_project(&file);

        let mut arguments = Arguments::build_args(
            false,
            RunnerSettings {
                sources: vec![],
                compiler_arguments: CompilerArguments { compiler: "llvm".to_string(), ..Default::default() },
            },
        );

        self.parents.insert(file.clone(), build_project::<()>(&mut arguments, &mut directory, false).unwrap().0);
        return self.parents.get(&file).unwrap().clone();
    }

    fn get_project(file: &PathBuf) -> Vec<Box<dyn SourceSet>> {
        let mut directory = file.parent();
        while let Some(dir) = directory {
            if dir.join("build.rv").exists() {
                break;
            }
            directory = dir.parent();
        }
        return vec![Box::new(FileSourceSet { root: directory.map(|inner| inner.to_path_buf()).unwrap_or(file.clone()) })];
    }
}
