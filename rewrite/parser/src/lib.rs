pub mod steps;

use std::any::TypeId;
use std::collections::HashMap;

use crate::steps::{ParserTopStep, ParserUtil};
use lexer::{lex, Token};
use new_runner::{RavenTask, TaskManager};
use program::ProgramAccess;
use types::{ComparableType, RawType};

#[derive(Default)]
pub struct ParserTasks {}

impl TaskManager for ParserTasks {
    fn setup(
        &mut self,
        raw_tasks: &mut Vec<Box<dyn RavenTask<RawType>>>,
        _finalizing_tasks: &mut Vec<Box<dyn RavenTask<dyn ComparableType>>>,
    ) {
        raw_tasks.push(Box::new(Parser::default()));
    }

    fn finish_setup(
        &mut self,
        _raw_tasks: &mut Vec<Box<dyn RavenTask<RawType>>>,
        _finalizing_tasks: &mut Vec<Box<dyn RavenTask<dyn ComparableType>>>,
    ) {
        // Not used
    }
}

#[derive(Default)]
pub struct Parser {
    pub keywords: HashMap<u64, &'static str>,
    steps: Vec<Box<dyn ParserTopStep>>,
}

impl Parser {
    pub fn add_step<T: ParserTopStep + 'static>(&mut self, step: T) {
        self.steps.push(Box::new(step));
    }
}

impl RavenTask<RawType> for Parser {
    fn run(&self, program: &mut dyn ProgramAccess<RawType>) {
        let sources = program.lock().sources.clone();
        for source in &*sources {
            for file in source.get_files() {
                let file = file.read();
                parse(program, &self.steps, &lex(&self.keywords, file.as_slice()), file.as_slice());
            }
        }
    }

    fn name(&self) -> &'static str {
        return "parser";
    }

    fn id(&self) -> TypeId {
        return TypeId::of::<Parser>();
    }
}

pub fn parse(
    program: &dyn ProgramAccess<RawType>,
    top_steps: &Vec<Box<dyn ParserTopStep>>,
    tokens: &Vec<Token>,
    file: &[u8],
) {
    let mut parser = ParserUtil { file, tokens, program, index: 0 };
    while parser.index < tokens.len() {
        for step in top_steps.iter() {
            if step.try_parse(&mut parser) {
                break;
            }
        }
    }
}
