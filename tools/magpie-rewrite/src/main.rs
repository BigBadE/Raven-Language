use functions::FunctionTask;
use new_parser::ParserTasks;
use new_runner::RavenRunner;
use std::path::PathBuf;
use std::rc::Rc;
use util::source::FileSourceSet;

fn main() {
    let mut runner = RavenRunner {
        task_managers: vec![Box::new(ParserTasks::default()), Box::new(FunctionTask::default())],
        sources: Rc::new(vec![Box::new(FileSourceSet::new(PathBuf::from("tools/magpie-rewrite/lib/build.rv")))]),
        ..Default::default()
    };
    runner.setup();
    runner.run();
}
