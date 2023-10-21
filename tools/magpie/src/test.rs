use std::path;
use include_dir::File;
use data::{Readable, SourceSet};
use crate::FileWrapper;

#[cfg(test)]
mod test {
    use std::path;
    use include_dir::{Dir, include_dir};
    use data::{Arguments, RunnerSettings};
    use crate::build;
    use crate::test::InnerFileSourceSet;

    static TESTS: Dir = include_dir!("lib/test/test");

    #[test]
    pub fn test_magpie() {
        test_recursive(&TESTS);
    }

    fn test_recursive(dir: &'static Dir) {
        for file in dir.entries() {
            if let Some(found) = file.as_file() {
                let mut arguments = Arguments::build_args(false, RunnerSettings {
                    sources: vec!(),
                    debug: false,
                    compiler: "llvm".to_string(),
                });

                let path = found.path().to_str().unwrap().replace(path::MAIN_SEPARATOR, "::");
                let path = format!("{}::test", &path[0..path.len()-3]);
                match build::<bool>(path.clone(), &mut arguments, vec!(Box::new(InnerFileSourceSet {
                    set: found
                }))) {
                    Ok(inner) => match inner {
                        Some(found) => if !found {
                            panic!("Failed test {}!", path)
                        },
                        None => panic!("Failed to find method test in test {}", path)
                    },
                    Err(()) => panic!("Failed to compile test {}!", path)
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
        return vec!(Box::new(FileWrapper { file: self.set }));
    }

    fn relative(&self, other: &Box<dyn Readable>) -> String {
        let name = other.path()
            .replace(path::MAIN_SEPARATOR, "::");
        return name[0..name.len() - 3].to_string();
    }
}