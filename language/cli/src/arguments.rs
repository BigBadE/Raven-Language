use std::collections::HashMap;
use std::{env, fs};
use std::env::Args;
use std::path::PathBuf;
use tokio::runtime::Builder;
use runner::{RunnerSettings, SourceSet};

pub struct Arguments {
    pub runner_settings: RunnerSettings,
}

impl Arguments {
    pub fn from_arguments(mut arguments: Args) -> Self {
        let mut all_args = HashMap::new();
        all_args.insert(ArgumentTypes::Runner, HashMap::new());
        let mut last: Option<(ArgumentTypes, String)> = None;

        //Skip the first arg (running location)
        arguments.next();

        let str_args = arguments.next().unwrap();
        for arg in str_args[0..str_args.len()].split(" ") {
            if arg.is_empty() {
                continue;
            }

            let arg = arg.to_string();
            if arg.starts_with("--") {
                if let Some((types, name)) = last {
                    let found = all_args.get_mut(&types).unwrap();
                    found.insert(name, vec!("true".to_string()));
                }
                last = Some((ArgumentTypes::Runner, arg[2..].to_string()))
            } else {
                last = match last {
                    Some((types, name)) => {
                        let modifying: &mut HashMap<String, Vec<String>> = all_args.get_mut(&types).unwrap();
                        if let Some(vec) = modifying.get_mut(&name) {
                            vec.push(arg);
                        } else {
                            modifying.insert(name.clone(), vec!(arg));
                        }
                        None
                    }
                    None => {
                        panic!("Unknown argument type: {}\n", arg)
                    }
                };
            }
        }

        let runner_args = all_args.get_mut(&ArgumentTypes::Runner).unwrap();
        if let Some(test) = runner_args.get("test") {
            match test.get(0).unwrap().as_str() {
                "ten_mil_lines" => {
                    println!("Writing test file:");
                    let test_folder = env::temp_dir().join("raven_test");
                    fs::create_dir_all(test_folder.clone()).unwrap();
                    let test_file = test_folder.clone().join("raven_test.rv");
                    fs::write(test_file.clone(), format!("pub internal struct i64 {{}} pub fn main() -> i64 {{{}return 123;}}",
                    "let a = 1;".repeat(2))).unwrap();
                    runner_args.insert("root".to_string(), vec!(test_folder.to_str().unwrap().to_string()));
                    println!("Test file written to {:?}", test_file);
                }
                test => panic!("Unknown test {}", test)
            }
        }
        return Self {
            runner_settings: Self::parse_runner_settings(runner_args)
        };
    }

    fn parse_runner_settings(arguments: &HashMap<String, Vec<String>>) -> RunnerSettings {
        return RunnerSettings {
            io_runtime: Builder::new_multi_thread().thread_name("io-runtime").build()
                .expect("Failed to build I/O runtime"),
            cpu_runtime: Builder::new_multi_thread().thread_name("cpu-runtime").build()
                .expect("Failed to build CPU runtime"),
            sources: arguments.get("root").expect("Need a source root, \
            pass it with the \"--root (root)\" argument").iter()
                .map(|root| SourceSet { root: PathBuf::from(root) }).collect(),
            debug: arguments.get("debug").is_some(),
            compiler: "llvm".to_string(),
        };
    }
}

#[derive(Eq, PartialOrd, PartialEq, Hash)]
enum ArgumentTypes {
    Runner
}