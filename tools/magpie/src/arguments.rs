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
        all_args.insert(ArgumentTypes::Magpie, HashMap::new());
        let mut last: Option<(ArgumentTypes, String)> = None;

        //Skip the first arg (running location)
        arguments.next();

        if arguments.len() == 0 {
            let runner_args = all_args.get_mut(&ArgumentTypes::Runner).unwrap();
            return Self {
                runner_settings: Self::parse_runner_settings(runner_args)
            };
        }

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
                        let args = all_args.get_mut(&ArgumentTypes::Magpie).unwrap();
                        if let Some(found) = args.get_mut("") {
                            found.push(arg);
                        } else {
                            args.insert(String::new(), vec!(arg));
                        }
                        None
                    }
                };
            }
        }

        let runner_args = all_args.get_mut(&ArgumentTypes::Runner).unwrap();
        return Self {
            runner_settings: Self::parse_runner_settings(runner_args)
        };
    }

    fn parse_runner_settings(arguments: &HashMap<String, Vec<String>>) -> RunnerSettings {
        let (mut io_runtime, mut cpu_runtime) = if arguments.get("single-threaded").is_some() {
            (Builder::new_current_thread(), Builder::new_current_thread())
        } else {
            (Builder::new_multi_thread(), Builder::new_multi_thread())
        };
        return RunnerSettings {
            io_runtime: io_runtime.thread_name("io-runtime").build()
                .expect("Failed to build I/O runtime"),
            cpu_runtime: cpu_runtime.thread_name("cpu-runtime").build()
                .expect("Failed to build CPU runtime"),
            sources: arguments.get("root").map(|inner| inner.iter()
                .map(|root| SourceSet { root: PathBuf::from(root) }).collect()).unwrap_or(Vec::new()),
            debug: arguments.get("debug").is_some(),
            compiler: "llvm".to_string(),
        };
    }
}

#[derive(Eq, PartialOrd, PartialEq, Hash)]
enum ArgumentTypes {
    Runner,
    Magpie
}