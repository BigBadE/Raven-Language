use crate::tokens::code_tokenizer::next_code_token;
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::top_tokenizer::next_top_token;

pub struct Tokenizer<'a> {
    pub state: Vec<TokenizerState>,
    pub index: usize,
    pub last: Token,
    pub len: usize,
    pub buffer: &'a [u8],
}

impl<'a> Tokenizer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        return Tokenizer {
            state: vec!(TokenizerState::TopElement),
            index: 0,
            last: Token::new(TokenTypes::Start, 0, 0),
            len: buffer.len(),
            buffer,
        };
    }

    pub fn next(&mut self) -> Token {
        return match self.state.get(0).unwrap() {
            TokenizerState::String => self.next_string(),
            TokenizerState::TopElement => next_top_token(self),
            _ => next_code_token(self),
        };
    }

    fn next_string(&mut self) -> Token {
        loop {
            if self.next_included()? == b'"' && self.last.token_type != TokenTypes::StringEscape {
                self.state.pop();
                return Token::new(TokenTypes::StringEnd, self.last.end + 1, self.index + 1);
            }
        }
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

    pub fn handle_invalid(&mut self) -> Token {
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
}

#[derive(Clone)]
pub enum TokenizerState {
    String = 0,
    TopElement = 1,
    Structure = 2,
    Function = 3,
    Code = 4,
}