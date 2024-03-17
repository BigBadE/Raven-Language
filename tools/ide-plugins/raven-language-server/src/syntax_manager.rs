use data::{Arguments, CompilerArguments, RunnerSettings};
use magpie_lib::setup_arguments;
use parking_lot::Mutex;
use parser::FileSourceSet;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax::program::syntax::Syntax;

#[derive(Default)]
pub struct SyntaxManager {
    pub parents: HashMap<PathBuf, Arc<Mutex<Syntax>>>,
}

impl SyntaxManager {
    pub fn get_syntax<'a>(&self, file: PathBuf) -> &Arc<Mutex<Syntax>> {
        if let Some(found) = self.parents.get(&file) {
            return found;
        }
        return self.update_syntax(file);
    }

    pub fn update_syntax(&self, file: PathBuf) -> &Arc<Mutex<Syntax>> {
        let mut directory = Self::get_project(file);
        let mut project = true;
        if directory.is_none() {
            directory = Some(&*env::temp_dir().join(format!("{}_temp", file.file_name().unwrap().to_str().unwrap())));
            project = false;
        }
        let mut arguments = Arguments::build_args(
            false,
            RunnerSettings {
                sources: vec![],
                compiler_arguments: CompilerArguments {
                    target: "unused".to_string(),
                    compiler: "llvm".to_string(),
                    temp_folder: directory.unwrap().join("target"),
                },
            },
        );

        let mut sources = vec![Box::new(FileSourceSet { root: directory.unwrap().to_path_buf() })];
        setup_arguments(&mut arguments, &mut sources);
    }

    fn get_project<'a>(file: PathBuf) -> Option<&'a Path> {
        let mut directory = file.parent();
        while let Some(dir) = directory {
            if dir.join("build.rv").exists() {
                break;
            }
            directory = dir.parent();
        }
        return directory;
    }
}
