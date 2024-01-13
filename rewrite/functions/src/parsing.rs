use crate::{BRACKET_CLOSE_KEYWORD, BRACKET_OPEN_KEYWORD, FN_KEYWORD, PAREN_CLOSE_KEYWORD, PAREN_OPEN_KEYWORD};
use lexer::VARIABLE;
use new_parser::steps::{KeywordParser, ParserStep, ParserTopStep, ParserUtil};

#[derive(Default)]
pub struct FunctionParsingData {}

pub struct FunctionParserStep {
    steps: Vec<Box<dyn ParserStep<FunctionParsingData>>>,
}

impl Default for FunctionParserStep {
    fn default() -> Self {
        return Self {
            steps: vec![
                Box::new(KeywordParser::new(FN_KEYWORD)),
                Box::new(KeywordParser::new(VARIABLE)),
                Box::new(KeywordParser::new(PAREN_OPEN_KEYWORD)),
                Box::new(KeywordParser::new(PAREN_CLOSE_KEYWORD)),
                Box::new(KeywordParser::new(BRACKET_OPEN_KEYWORD)),
                Box::new(KeywordParser::new(BRACKET_CLOSE_KEYWORD)),
            ],
        };
    }
}

impl ParserTopStep for FunctionParserStep {
    fn try_parse(&self, parser_util: &mut ParserUtil) -> bool {
        let mut data = FunctionParsingData::default();
        for step in &self.steps {
            if !step.try_parse(parser_util, &mut data) {
                return false;
            }
        }
        return true;
    }
}
