use lexer::Token;
use program::ProgramAccess;
use std::collections::HashMap;
use types::RawType;

pub struct ParserUtil<'a> {
    pub file: &'a [u8],
    pub index: usize,
    pub tokens: &'a Vec<Token>,
    pub program: &'a dyn ProgramAccess<RawType>,
}

pub trait ParserTopStep {
    fn try_parse(&self, parser_util: &mut ParserUtil) -> bool;
}

pub trait ParserStep<T> {
    fn try_parse(&self, parser_util: &mut ParserUtil, data: &mut T) -> bool;
}

pub struct KeywordParser {
    keyword: u64,
}

impl KeywordParser {
    pub fn new(keyword: u64) -> Self {
        return Self { keyword };
    }
}

impl<T> ParserStep<T> for KeywordParser {
    fn try_parse(&self, parser_util: &mut ParserUtil, _data: &mut T) -> bool {
        if parser_util.tokens[parser_util.index].id == self.keyword {
            parser_util.index += 1;
            return true;
        }
        return false;
    }
}

pub struct WordParser {
    keyword: u64,
}

impl<T> ParserStep<T> for WordParser {
    fn try_parse(&self, parser_util: &mut ParserUtil, _data: &mut T) -> bool {
        if parser_util.tokens[parser_util.index].id == self.keyword {
            parser_util.index += 1;
            return true;
        }
        return false;
    }
}
