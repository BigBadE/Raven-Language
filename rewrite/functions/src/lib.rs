use crate::parsing::FunctionParserStep;
use new_parser::Parser;
use new_runner::{Downcastable, MapWrapper, RavenTask, TaskManager};
use program::ProgramComponent;
use std::cell::RefCell;
use std::ops::DerefMut;
use std::rc::Rc;
use types::{ComparableType, RawType, Type};
use util::top_elements::{TopElement, TopElementManager};

mod parsing;

pub const FN_KEYWORD: u64 = 10000;
pub const PAREN_OPEN_KEYWORD: u64 = 10001;
pub const PAREN_CLOSE_KEYWORD: u64 = 10002;
pub const BRACKET_OPEN_KEYWORD: u64 = 10003;
pub const BRACKET_CLOSE_KEYWORD: u64 = 10004;

//pub struct FunctionHeader<T: Type> {}

//pub struct Function<T: Type> {}

#[derive(Default)]
pub struct FunctionTask {}

impl TaskManager for FunctionTask {
    fn setup(
        &mut self,
        _raw_tasks: &mut Vec<Box<dyn RavenTask<RawType>>>,
        _finalizing_tasks: &mut Vec<Box<dyn RavenTask<dyn ComparableType>>>,
    ) {
        // Unused
    }

    fn finish_setup(
        &mut self,
        raw_tasks: &mut Vec<Box<dyn RavenTask<RawType>>>,
        _finalizing_tasks: &mut Vec<Box<dyn RavenTask<dyn ComparableType>>>,
    ) {
        let parser: &mut Parser = raw_tasks.downcast();
        parser.keywords.insert(FN_KEYWORD, "fn");
        parser.keywords.insert(PAREN_OPEN_KEYWORD, "(");
        parser.keywords.insert(PAREN_CLOSE_KEYWORD, ")");
        parser.keywords.insert(BRACKET_OPEN_KEYWORD, "{");
        parser.keywords.insert(BRACKET_CLOSE_KEYWORD, "}");
        parser.add_step(FunctionParserStep::default());
    }
}

/*impl TopElement for FunctionHeader<RawType> {
    type Finalized = Function<dyn ComparableType>;
}

pub struct FunctionComponent {
    pub manager: TopElementManager<FunctionHeader<RawType>>,
}

impl ProgramComponent for FunctionComponent {
    fn join(&mut self, other: &dyn ProgramComponent) {
        todo!()
    }
}*/
