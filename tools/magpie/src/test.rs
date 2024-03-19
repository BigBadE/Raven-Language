#[cfg(test)]
mod test {
    use data::{Arguments, CompilerArguments, RunnerSettings};
    use magpie_lib::build_project;
    use parser::FileSourceSet;
    use std::path::PathBuf;
    use std::{env, fs, path};

    /// Main test
    #[test]
    pub fn test_magpie() {
        let test_folder: PathBuf = ["..", "..", "lib", "test", "test"].iter().collect();
        test_recursive(test_folder);
    }

    /// Recursively searches for files in the test folder to run as a test
    fn test_recursive(path: PathBuf) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                // supposedly, this is a test file
                let mod_path = path.to_str().unwrap().replace(path::MAIN_SEPARATOR, "::");
                if !mod_path.ends_with(".rv") {
                    println!("File {} doesn't have the right file extension!", mod_path);
                    continue;
                }
                let mod_path =
                    format!("{}::test", &mod_path[path.parent().unwrap().to_str().unwrap().len() + 6..mod_path.len() - 3]);
                println!("Running {}", mod_path);
                let mut arguments = Arguments::build_args(
                    false,
                    RunnerSettings {
                        sources: vec![],
                        compiler_arguments: CompilerArguments {
                            compiler: "llvm".to_string(),
                            target: mod_path.clone(),
                            temp_folder: env::current_dir().unwrap().join("target"),
                        },
                    },
                );

                match build_project::<bool>(&mut arguments, &mut vec![Box::new(FileSourceSet { root: path })], true) {
                    Ok((_, inner)) => match inner {
                        Some(found) => {
                            if !found {
                                assert!(false, "Failed test {}!", mod_path)
                            }
                        }
                        None => assert!(false, "Failed to find method test in test {}", mod_path),
                    },
                    Err(()) => assert!(false, "Failed to compile test {}!", mod_path),
                }
            } else if path.is_dir() {
                // supposedly, this is a sub-directory in the test folder
                test_recursive(path);
            } else {
                println!("Unknown element in test folder!");
                continue;
            }
        }
    }
}
