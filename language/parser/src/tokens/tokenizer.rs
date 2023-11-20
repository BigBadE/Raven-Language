use crate::tokens::code_tokenizer::next_code_token;
use crate::tokens::tokens::{Token, TokenCodeData, TokenTypes};
use crate::tokens::top_tokenizer::{
    next_func_token, next_implementation_token, next_struct_token, next_top_token,
};
use crate::tokens::util::{next_generic, parse_string};

/// This structure keeps track of the variables required for the tokenizing.
pub struct Tokenizer<'a> {
    // The current state. This is used to determine which method will handle the next token.
    // See TokenizerState for all the states
    pub state: u64,
    // The depth of brackets
    pub bracket_depth: u8,
    // The depth of generics (within a <)
    pub generic_depth: u8,
    // The index in the character buffer
    pub index: usize,
    // The current line number
    pub line: u32,
    // The index from the beginning of the line.
    pub line_index: u32,
    // The last token that was parsed
    pub last: Token,
    // The length of the file
    pub len: usize,
    // A buffer of all characters in the file
    pub buffer: &'a [u8],
    // Data for token errors
    pub code_data: Option<TokenCodeData>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        return Tokenizer {
            state: TokenizerState::TOP_ELEMENT,
            bracket_depth: 0,
            generic_depth: 1,
            index: 0,
            line: 1,
            line_index: 0,
            last: Token::new(TokenTypes::Start, None, (1, 0), 0, (1, 0), 0),
            len: buffer.len(),
            buffer,
            code_data: None,
        };
    }

    /// Saves the tokenizer to a ParserState to be loaded later
    pub fn serialize(&mut self) -> ParserState {
        return ParserState {
            state: self.state,
            index: self.index,
            line_index: self.line_index,
            line: self.line,
            last: self.last.clone(),
        };
    }

    /// Loads the state from a ParserState
    pub fn load(&mut self, state: &ParserState) {
        self.state = state.state;
        self.index = state.index;
        self.line_index = state.line_index;
        self.line = state.line;
        self.last.clone_from(&state.last);
    }

    pub fn next(&mut self) -> Token {
        if self.matches("//") {
            self.parse_to_line_end(TokenTypes::Comment);
            self.last = self.make_token(TokenTypes::Comment);
            return self.last.clone();
        } else if self.matches("/*") {
            while !self.matches("*/") {
                self.index += 1;
            }
            self.last = self.make_token(TokenTypes::Comment);
            return self.last.clone();
        }

        self.last = match self.state {
            TokenizerState::TOP_ELEMENT | TokenizerState::TOP_ELEMENT_TO_STRUCT => {
                next_top_token(self)
            }
            TokenizerState::FUNCTION | TokenizerState::FUNCTION_TO_STRUCT_TOP => {
                next_func_token(self)
            }
            TokenizerState::STRUCTURE => next_struct_token(self),
            TokenizerState::IMPLEMENTATION => next_implementation_token(self),
            TokenizerState::STRING | TokenizerState::STRING_TO_CODE_STRUCT_TOP => {
                parse_string(self)
            }
            TokenizerState::CODE | TokenizerState::CODE_TO_STRUCT_TOP => next_code_token(self),
            TokenizerState::GENERIC_TO_IMPL
            | TokenizerState::GENERIC_TO_FUNC
            | TokenizerState::GENERIC_TO_STRUCT
            | TokenizerState::GENERIC_TO_FUNC_TO_STRUCT_TOP => next_generic(self),
            _ => panic!("Unknown state {}!", self.state),
        };
        return self.last.clone();
    }

    // The next included character, or the EOF token.
    // This allows the ? operator to automatically return if the end of the file is reached.
    pub fn next_included(&mut self) -> Result<u8, Token> {
        loop {
            if self.index == self.len {
                return Err(Token::new(
                    TokenTypes::EOF,
                    None,
                    self.last.end,
                    self.last.end_offset,
                    (self.line, self.index as u32 - self.line_index),
                    self.index,
                ));
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
                _ => return Ok(character),
            }
        }
    }

    /// Matches a string of characters to the current tokenizer index. Ignores whitespace.
    pub fn matches(&mut self, input: &str) -> bool {
        let state = self.serialize();
        for character in input.bytes() {
            let found = self.next_included().unwrap_or(b' ');
            if found != character {
                self.load(&state);
                return false;
            }
        }
        return true;
    }

    /// Matches a word. Unlike matches, this requires it to be word, which means it must end in a space
    pub fn matches_word(&mut self, input: &str) -> bool {
        let state = self.serialize();
        for character in input.bytes() {
            let found = self.next_included().unwrap_or(b' ');
            if found != character {
                self.load(&state);
                return false;
            }
        }
        return if !self.buffer[self.index].is_ascii_alphabetic() {
            true
        } else {
            self.load(&state);
            false
        };
    }

    /// Parse ahead to the first occurrence of whichever token occurs first
    pub fn parse_to_first(&mut self, token: TokenTypes, first: u8, second: u8) -> Token {
        while self.index != self.len
            && self.buffer[self.index] != first
            && self.buffer[self.index] != second
        {
            self.index += 1;
        }

        return Token::new(
            token,
            self.code_data.clone(),
            self.last.end,
            self.last.end_offset,
            (self.line, self.index as u32 - self.line_index),
            self.index,
        );
    }

    /// Parse ahead to the end of the current line
    pub fn parse_to_line_end(&mut self, types: TokenTypes) -> Token {
        if self.index == self.len {
            return Token::new(
                TokenTypes::EOF,
                self.code_data.clone(),
                self.last.end,
                self.last.end_offset,
                (self.line, self.index as u32 - self.line_index),
                self.index,
            );
        }

        loop {
            self.index += 1;
            if self.index == self.len || self.buffer[self.index] == b'\n' {
                break;
            }
        }

        return Token::new(
            types,
            self.code_data.clone(),
            self.last.end,
            self.last.end_offset,
            (self.line, self.index as u32 - self.line_index),
            self.index - 1,
        );
    }

    /// Creates an InvalidCharacters token, used for debugging (you can put a breakpoint here)
    pub fn handle_invalid(&mut self) -> Token {
        return self.parse_to_line_end(TokenTypes::InvalidCharacters);
    }

    /// Creates a token between the last token and the current position
    pub fn make_token(&self, token_type: TokenTypes) -> Token {
        return Token::new(
            token_type,
            self.code_data.clone(),
            self.last.end,
            self.last.end_offset,
            (self.line, self.index as u32 - self.line_index),
            self.index,
        );
    }
}

/// A serialized parser state, used to save/load the state of parsing mid-file.
pub struct ParserState {
    pub state: u64,
    pub index: usize,
    pub line_index: u32,
    pub line: u32,
    pub last: Token,
}

#[non_exhaustive]
pub struct TokenizerState {}

/// Constants for the different parser states.
/// Some states map to the same method, but just have
/// different names because they return to different states.
impl TokenizerState {
    // Parsing a string like "Test"
    pub const STRING: u64 = 0;
    // A string that returns to the code top. For example, in static variables.
    pub const STRING_TO_CODE_STRUCT_TOP: u64 = 1;
    // The top of the file, not inside anything.
    pub const TOP_ELEMENT: u64 = 2;
    // Inside a structure declaration
    pub const STRUCTURE: u64 = 3;
    // Inside an implementation declaration, turns into a STRUCTURE for the body
    pub const IMPLEMENTATION: u64 = 4;
    // Inside a function
    pub const FUNCTION: u64 = 5;
    // Inside a function that's inside a structure or impl
    pub const FUNCTION_TO_STRUCT_TOP: u64 = 6;
    // Inside the generic declaration of a function declaration.
    pub const GENERIC_TO_FUNC: u64 = 0x7;
    // Inside the generic declaration of a function declaration in a structure.
    pub const GENERIC_TO_FUNC_TO_STRUCT_TOP: u64 = 0x8;
    // Inside the generic declaration of a structure
    pub const GENERIC_TO_STRUCT: u64 = 0x9;
    // Inside the generic declaration of an implementation
    pub const GENERIC_TO_IMPL: u64 = 0xA;
    // The inside of a structure
    pub const TOP_ELEMENT_TO_STRUCT: u64 = 0xB;
    // A block of code
    pub const CODE: u64 = 0xC;
    // A block of code that returns to a structure
    pub const CODE_TO_STRUCT_TOP: u64 = 0xD;
}
