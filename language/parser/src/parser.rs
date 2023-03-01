use std::fmt::{Display, Formatter};
use ast::TopElement;
use crate::top_elements::parse_top_element;
use crate::util::get_line;

#[derive(Clone)]
pub struct ParseInfo<'a> {
    buffer: &'a [u8],
    pub index: usize,
    len: usize,
    line: u64,
    line_index: usize
}

impl<'a> ParseInfo<'a> {
    pub fn new(buffer: &'a String) -> Self {
        return Self {
            len: buffer.len() as usize,
            buffer: buffer.as_bytes(),
            index: 0,
            line: 0,
            line_index: 0
        }
    }

    pub fn find_next(&mut self, char: u8) -> bool {
        return match self.next_included() {
            Some(found) => found == char,
            None => false
        }
    }

    pub fn parse_to(&mut self, char: u8) -> Option<String> {
        let mut output = String::new();
        while let Some(character) = self.next_included() {
            if character == char {
                return Some(output);
            }
            output.push(character as char);
        }
        return None;
    }

    pub fn subparse(&mut self, ending_char: u8) -> Option<Self> {
        let mut end = 0;
        let mut temp = self.clone();
        while let Some(character) = temp.next_included() {
            if character == ending_char {
                end = temp.index;
                break;
            }
        }

        if end == 0 {
            return None;
        }

        return Some(Self {
            buffer: &self.buffer[..end],
            index: self.index,
            len: end,
            line: self.line,
            line_index: self.line_index
        });
    }

    pub fn next_included(&mut self) -> Option<u8> {
        while self.index < self.len {
            self.index += 1;
            if !self.whitespace_next(self.buffer[self.index]) {
                return Some(self.buffer[self.index-1]);
            }
        }
        return None;
    }

    pub fn matching(&mut self, matching: &str) -> bool {
        let saved = self.clone();

        for char in matching.as_bytes() {
            if char != match &self.next_included() {
                Some(found) => found,
                None => return false
            } {
                *self = saved;
                return false;
            }
        }

        return true;
    }

    pub fn create_error(&mut self, error: String) -> ParseError {
        let line = self.line;
        while line == self.line {
            self.next_included();
        }
        return ParseError::new(self.line, (self.index-self.line_index) as u64,
                               get_line(self.buffer, self.line_index), error);
    }

    fn whitespace_next(&mut self, char: u8) -> bool {
        return if char == b' ' || char == b'\t' || char == b'\r' {
            true
        } else if char == b'\n' {
            self.line += 1;
            self.line_index = self.index;
            true
        } else {
            false
        }
    }
}

pub struct ParseError {
    pub row: u64,
    pub column: u64,
    pub line: String,
    pub error: String
}

impl ParseError {
    pub fn new(row: u64, column: u64, line: String, error: String) -> Self {
        return Self {
            row,
            column,
            line,
            error
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "Error at {} ({}:{}): {}", self.line, self.row, self.column, self.error);
    }
}

pub fn parse(name: &String, input: String) -> Result<Vec<TopElement>, Vec<ParseError>> {
    let mut output = Vec::new();
    let mut parsing = ParseInfo::new(&input);
    let mut errors = Vec::new();

    while let Some(element) = parse_top_element(name, &mut errors, &mut parsing) {
        output.push(element);
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    return Ok(output);
}