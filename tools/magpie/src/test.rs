use crate::FileWrapper;
use data::{Readable, SourceSet};
use include_dir::File;
use std::path;

#[cfg(test)]
mod test {
    use crate::build;
    use crate::test::InnerFileSourceSet;
    use data::{Arguments, CompilerArguments, RunnerSettings};
    use include_dir::{include_dir, Dir, DirEntry};
    use std::{env, path};

    static TESTS: Dir = include_dir!("lib/test/test");

    #[test]
    pub fn test_magpie() {
        test_recursive(&TESTS);
    }

    fn test_recursive(dir: &'static Dir<'_>) {
        for entry in dir.entries() {
            match entry {
                DirEntry::File(file) => {
                    let path = file
                        .path()
                        .to_str()
                        .unwrap()
                        .replace(path::MAIN_SEPARATOR, "::");
                    println!("Running {}", path);
                    if !path.ends_with(".rv") {
                        println!("File {} doesn't have the right file extension!", path);
                        continue;
                    }
                    let path = format!("{}::test", &path[0..path.len() - 3]);
                    let mut arguments = Arguments::build_args(
                        false,
                        RunnerSettings {
                            sources: vec![],
                            debug: false,
                            compiler_arguments: CompilerArguments {
                                compiler: "llvm".to_string(),
                                target: path.clone(),
                                temp_folder: env::current_dir().unwrap().join("target"),
                            },
                        },
                    );

                    match build::<bool>(
                        &mut arguments,
                        vec![Box::new(InnerFileSourceSet { set: file })],
                    ) {
                        Ok(inner) => match inner {
                            Some(found) => {
                                if !found {
                                    assert!(false, "Failed test {}!", path)
                                }
                            }
                            None => assert!(false, "Failed to find method test in test {}", path),
                        },
                        Err(()) => assert!(false, "Failed to compile test {}!", path),
                    }
                }
                DirEntry::Dir(dir) => {
                    test_recursive(dir);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct InnerFileSourceSet {
    set: &'static File<'static>,
}

impl SourceSet for InnerFileSourceSet {
    fn get_files(&self) -> Vec<Box<dyn Readable>> {
        return vec![Box::new(FileWrapper { file: self.set })];
    }

    fn relative(&self, other: &dyn Readable) -> String {
        let name = other.path().replace(path::MAIN_SEPARATOR, "::");
        return name[0..name.len() - 3].to_string();
    }

    fn cloned(&self) -> Box<dyn SourceSet> {
        return Box::new(self.clone());
    }
}
