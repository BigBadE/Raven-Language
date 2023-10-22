use std::path;
use include_dir::File;
use data::{Readable, SourceSet};
use crate::FileWrapper;

#[cfg(test)]
mod test {
    use std::path;
    use include_dir::{Dir, DirEntry, include_dir};
    use data::{Arguments, RunnerSettings};
    use crate::build;
    use crate::test::InnerFileSourceSet;

    static TESTS: Dir = include_dir!("lib/test/test");

    #[test]
    pub fn test_magpie() {
        test_recursive(&TESTS);
        println!("Finished test!");
    }

    fn test_recursive(dir: &'static Dir) {
        for file in dir.entries() {
            println!("Starting entry!");
            println!("Done!");
            match file {
                DirEntry::File(file) => {
                    println!("Starting with {}", file.path().to_str().unwrap());
                    let mut arguments = Arguments::build_args(false, RunnerSettings {
                        sources: vec!(),
                        debug: false,
                        compiler: "llvm".to_string(),
                    });

                    let path = file.path().to_str().unwrap().replace(path::MAIN_SEPARATOR, "::");
                    let path = format!("{}::test", &path[0..path.len() - 3]);
                    match build::<bool>(path.clone(), &mut arguments, vec!(Box::new(InnerFileSourceSet {
                        set: file
                    }))) {
                        Ok(inner) => match inner {
                            Some(found) => if !found {
                                assert!(false, "Failed test {}!", path)
                            },
                            None => assert!(false, "Failed to find method test in test {}", path)
                        },
                        Err(()) => assert!(false, "Failed to compile test {}!", path)
                    }
                    println!("Passed {}", path);
                }
                DirEntry::Dir(dir) => {
                    println!("Recursing!");
                    test_recursive(dir);
                    println!("Done recursing!")
                }
            }
            println!("Done with entry!");
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