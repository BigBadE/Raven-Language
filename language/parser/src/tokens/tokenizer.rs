use crate::tokens::code_tokenizer::next_code_token;
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::top_tokenizer::{next_func_token, next_implementation_token, next_struct_token, next_top_token};
use crate::tokens::util::{next_generic, next_string};

pub struct Tokenizer<'a> {
    pub state: u64,
    pub index: usize,
    pub line: u32,
    pub line_index: u32,
    pub last: Token,
    pub len: usize,
    pub buffer: &'a [u8],
    pub for_loop: bool
}

impl<'a> Tokenizer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        return Tokenizer {
            state: TokenizerState::TOP_ELEMENT,
            index: 0,
            line: 1,
            line_index: 0,
            last: Token::new(TokenTypes::Start, (0, 0), 0, (0, 0), 0),
            len: buffer.len(),
            buffer,
            for_loop: false
        };
    }

    pub fn serialize(&mut self) -> ParserState {
        return ParserState {
            state: self.state.clone(),
            index: self.index.clone(),
            line_index: self.line_index.clone(),
            line: self.line.clone(),
            last: self.last.clone(),
        };
    }

    pub fn load(&mut self, state: &ParserState) {
        self.state = state.state.clone();
        self.index = state.index.clone();
        self.line_index = state.line_index.clone();
        self.line = state.line.clone();
        self.last = state.last.clone();
    }

    pub fn next(&mut self) -> Token {
        if self.matches("//") {
            return self.parse_to_line_end(TokenTypes::Comment);
        } else if self.matches("/*") {
            while !self.matches("*/") {

            }
            return self.make_token(TokenTypes::Comment);
        }
        self.last = match self.state {
            TokenizerState::GENERIC_TO_FUNC | TokenizerState::GENERIC_TO_FUNC_TOP |
            TokenizerState::GENERIC_TO_STRUCT | TokenizerState::GENERIC_TO_IMPL => next_generic(self),
            TokenizerState::TOP_ELEMENT | TokenizerState::TOP_ELEMENT_TO_STRUCT => next_top_token(self),
            TokenizerState::FUNCTION | TokenizerState::FUNCTION_TO_STRUCT_TOP => next_func_token(self),
            TokenizerState::STRUCTURE => next_struct_token(self),
            TokenizerState::IMPLEMENTATION => next_implementation_token(self),
            state =>
                if state & 0xFF <= 1 {
                    next_string(self)
                } else {
                    next_code_token(self, state & 0xFFFFFF00)
                },
        };
        return self.last.clone();
    }

    pub fn next_included(&mut self) -> Result<u8, Token> {
        loop {
            if self.index == self.len {
                return Err(Token::new(TokenTypes::EOF, self.last.end, self.last.end_offset,
                                      (self.line, self.index as u32 - self.line_index), self.index));
            }
            let character = self.buffer[self.index];
            self.index += 1;
            match character {
                b' ' => {}
                b'\n' => {
                    self.line_index = self.index as u32;
                    self.line += 1;
                }
                b'\r' => {}
                b'\t' => {}
                _ => return Ok(character)
            }
        }
    }

    pub fn matches(&mut self, input: &str) -> bool {
        let start = self.index;
        let offset = self.line_index;
        let line = self.line;
        for character in input.bytes() {
            if self.next_included().unwrap_or(b' ') != character {
                self.index = start;
                self.line_index = offset;
                self.line = line;
                return false;
            }
        }
        return true;
    }

    pub fn matches_nospace(&mut self, input: &str) -> bool {
        let mut start = self.index;
        for character in input.bytes() {
            if start == self.len || self.buffer[start] != character {
                return false;
            }
            start += 1;
        }
        self.matches(input);
        return true;
    }

    pub fn parse_to_first(&mut self, token: TokenTypes, first: u8, second: u8) -> Token {
        while self.index != self.len && self.buffer[self.index] != first && self.buffer[self.index] != second {
            self.index += 1;
        }

        return Token::new(token, self.last.end, self.last.end_offset,
                          (self.line, self.index as u32 - self.line_index), self.index);
    }

    pub fn parse_to_line_end(&mut self, types: TokenTypes) -> Token {
        if self.index == self.len {
            return Token::new(TokenTypes::EOF, self.last.end, self.last.end_offset,
                              (self.line, self.index as u32 - self.line_index), self.index);
        }

        loop {
            self.index += 1;
            if self.index == self.len || self.buffer[self.index] == b'\n' {
                self.line_index = self.index as u32;
                self.line += 1;
                break;
            }
        }

        return Token::new(types, self.last.end, self.last.end_offset,
                          (self.line, self.index as u32 - self.line_index), self.index - 1);
    }

    pub fn handle_invalid(&mut self) -> Token {
        return self.parse_to_line_end(TokenTypes::InvalidCharacters);
    }

    pub fn make_token(&self, token_type: TokenTypes) -> Token {
        return Token::new(token_type, self.last.end, self.last.end_offset,
                          (self.line, self.index as u32 - self.line_index), self.index);
    }
}

pub struct ParserState {
    pub state: u64,
    pub index: usize,
    pub line_index: u32,
    pub line: u32,
    pub last: Token
}

#[non_exhaustive]
pub struct TokenizerState {}

impl TokenizerState {
    pub const STRING: u64 = 0;
    pub const STRING_TO_CODE_STRUCT_TOP: u64 = 1;
    pub const TOP_ELEMENT: u64 = 2;
    pub const STRUCTURE: u64 = 3;
    pub const IMPLEMENTATION: u64 = 4;
    pub const FUNCTION: u64 = 5;
    pub const FUNCTION_TO_STRUCT_TOP: u64 = 6;
    pub const GENERIC_TO_FUNC: u64 = 7;
    pub const GENERIC_TO_FUNC_TOP: u64 = 8;
    pub const GENERIC_TO_STRUCT: u64 = 9;
    pub const GENERIC_TO_IMPL: u64 = 10;
    pub const TOP_ELEMENT_TO_STRUCT: u64 = 11;
    pub const CODE: u64 = 12;
    pub const CODE_TO_STRUCT_TOP: u64 = 13;
}