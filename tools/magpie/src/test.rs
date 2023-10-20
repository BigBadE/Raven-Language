use std::path;
use include_dir::File;
use data::{Readable, SourceSet};

#[cfg(test)]
mod test {
    use include_dir::Dir;
    use data::{Arguments, RunnerSettings};
    use crate::build;

    #[test]
    pub fn test_magpie() {

    }

    fn test_recursive(dir: &Dir) {
        for file in dir.entries() {
            if let Some(found) = file.as_file() {
                let mut arguments = Arguments::build_args(false, RunnerSettings {
                    sources: vec!(),
                    debug: false,
                    compiler: "llvm".to_string(),
                });

                match build::<()>("main::main".to_string(), &mut arguments, vec!(Box::new(InternalFileSourceSet {
                    root: source
                }))) {
                    _ => {}
                }
            } else {
                test_recursive(file.as_dir().unwrap());
            }
        }
    }
}

#[derive(Debug)]
pub struct InnerFileSourceSet {
    set: &'static File<'static>,
}

impl SourceSet for InnerFileSourceSet {
    fn get_files(&self) -> Vec<Box<dyn Readable>> {
        return vec!(Box::new(self.set));
    }

    fn relative(&self, other: &Box<dyn Readable>) -> String {
        let name = other.path()
            .replace(path::MAIN_SEPARATOR, "::");
        return name[0..name.len() - 3].to_string();
    }
}