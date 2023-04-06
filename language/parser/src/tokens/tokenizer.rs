use crate::tokens::code_tokenizer::next_code_token;
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::top_tokenizer::{next_func_token, next_implementation_token, next_struct_token, next_top_token};
use crate::tokens::util::{next_generic, next_string};

pub struct Tokenizer<'a> {
    pub state: TokenizerState,
    pub index: usize,
    pub last: Token,
    pub len: usize,
    pub buffer: &'a [u8],
}

impl<'a> Tokenizer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        return Tokenizer {
            state: TokenizerState::TopElement,
            index: 0,
            last: Token::new(TokenTypes::Start, 0, 0),
            len: buffer.len(),
            buffer,
        };
    }

    pub fn serialize(&mut self) -> ParserState {
        return ParserState {
            state: self.state.clone(),
            index: self.index.clone(),
            last: self.last.clone()
        }
    }

    pub fn load(&mut self, state: &ParserState) {
        self.state = state.state.clone();
        self.index = state.index.clone();
        self.last = state.last.clone();
    }

    pub fn next(&mut self) -> Token {
        self.last = match self.state {
            TokenizerState::String | TokenizerState::StringToCodeStructTop => next_string(self),
            TokenizerState::GenericToFunc | TokenizerState::GenericToStruct
            | TokenizerState::GenericToImpl => next_generic(self),
            TokenizerState::TopElement | TokenizerState::TopElementToStruct => next_top_token(self),
            TokenizerState::Function | TokenizerState::FunctionToStructTop => next_func_token(self),
            TokenizerState::Structure => next_struct_token(self),
            TokenizerState::Implementation => next_implementation_token(self),
            TokenizerState::Code | TokenizerState::CodeToStructTop => next_code_token(self),
        };
        return self.last.clone();
    }

    pub fn last(&self) -> u8 {
        return self.buffer[self.index-1];
    }

    pub fn next_included(&mut self) -> Result<u8, Token> {
        loop {
            if self.index == self.len {
                return Err(Token::new(TokenTypes::EOF, self.index, self.index));
            }
            let character = self.buffer[self.index];
            self.index += 1;
            match character {
                b' ' => {}
                b'\n' => {}
                b'\r' => {}
                b'\t' => {}
                _ => return Ok(character)
            }
        }
    }

    pub fn matches(&mut self, input: &str) -> bool {
        let start = self.index;
        for character in input.bytes() {
            if self.next_included().unwrap_or(b' ') != character {
                self.index = start;
                return false;
            }
        }
        return true;
    }

    pub fn parse_to_first(&mut self, token: TokenTypes, first: u8, second: u8) -> Token {
        while self.index != self.len && self.buffer[self.index] != first && self.buffer[self.index] != second {
            self.index += 1;
        }

        return Token::new(token, self.last.end, self.index);
    }

    pub fn handle_invalid(&mut self) -> Token {
        if self.index == self.len {
            return Token::new(TokenTypes::EOF, self.index, self.index);
        }

        while self.index != self.len {
            if self.buffer[self.index] == b'\n' {
                break
            }
            self.index += 1;
        }
        return Token::new(TokenTypes::InvalidCharacters, self.last.end, self.index-1);
    }

    pub fn make_token(&self, token_type: TokenTypes) -> Token {
        return Token::new(token_type, self.last.end, self.index);
    }

    pub fn parse_to_next_space(&mut self, token_type: TokenTypes) -> Token {
        while self.buffer[self.index] != b' ' {
            self.index += 1;
        }
        return Token::new(token_type, self.last.end, self.index);
    }
}

pub struct ParserState {
    pub state: TokenizerState,
    pub index: usize,
    pub last: Token
}

#[derive(Clone, PartialEq, Debug)]
pub enum TokenizerState {
    String = 0,
    StringToCodeStructTop = 1,
    TopElement = 2,
    Structure = 3,
    Implementation = 4,
    Function = 5,
    FunctionToStructTop = 6,
    Code = 7,
    CodeToStructTop = 8,
    GenericToFunc = 9,
    GenericToStruct = 10,
    GenericToImpl = 11,
    TopElementToStruct = 12
}