use std::collections::HashMap;
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
        for arg in str_args[0..str_args.len() - 1].split(" ") {
            if arg.is_empty() {
                continue;
            }

            let arg = arg.to_string();
            if arg.starts_with("--") {
                if let Some((types, name)) = last {
                    let mut found = all_args.get_mut(&types).unwrap();
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
        return Self {
            runner_settings: Self::parse_runner_settings(all_args.get(&ArgumentTypes::Runner).unwrap())
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