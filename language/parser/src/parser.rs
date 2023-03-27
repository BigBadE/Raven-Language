use std::fmt::{Display, Formatter};
use ast::type_resolver::TypeResolver;
use crate::top_elements::{parse_top_elements};
use crate::util::get_line;

#[derive(Clone)]
pub struct ParseInfo<'a> {
    pub errors: Vec<ParseError>,
    pub buffer: &'a [u8],
    pub index: usize,
    pub indent: String,
    pub len: usize,
    line: u32,
    line_index: usize
}

impl<'a> ParseInfo<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        return Self {
            errors: Vec::new(),
            len: buffer.len() as usize,
            buffer,
            index: 0,
            indent: String::new(),
            line: 1,
            line_index: 0
        }
    }

    pub fn find_next(&mut self, char: u8) -> bool {
        return match self.next_included() {
            Some(found) => found == char,
            None => false
        }
    }

    pub fn find_end(&mut self) {
        while self.index < self.len {
            self.index += 1;
            match self.buffer[self.index] {
                b'"' => self.find_end_str(),
                b'{' => self.find_end(),
                b'}' => {
                    self.index += 1;
                    return
                },
                _ => {}
            }
        }
    }

    pub fn find_end_str(&mut self) {
        let mut ignoring = false;
        while self.index < self.len {
            self.index += 1;
            match self.buffer[self.index] {
                b'"' => if ignoring {
                    ignoring = false
                } else {
                    self.index += 1;
                    return;
                },
                b'\\' => ignoring = true,
                _ => if ignoring {
                    ignoring = false
                }
            }
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

    pub fn parse_to_space(&mut self) -> Option<String> {
        let mut output = String::new();
        while self.index < self.len {
            let character = self.buffer[self.index];
            self.index += 1;
            if self.whitespace_next(character) {
                return Some(output);
            }
            output.push(character as char);
        }
        return None;
    }

    pub fn parse_to_or_end(&mut self, char: u8, end: usize) -> Option<String> {
        let mut output = String::new();
        while self.index < end {
            if let Some(character) = self.next_included() {
                if character == char {
                    return Some(output);
                }
                output.push(character as char);   
            } else {
                return None
            }
        }
        return None;
    }
    
    pub fn next_included(&mut self) -> Option<u8> {
        while self.index < self.len {
            if !self.whitespace_next(self.buffer[self.index]) {
                if self.buffer[self.index] == b'/' {
                    if self.index < self.len-1 && self.buffer[self.index+1] == b'*' {
                        self.parse_comment();
                    } else if self.index < self.len-1 && self.buffer[self.index+1] == b'/' {
                        self.skip_line();
                    } else {
                        self.index += 1;
                        return Some(self.buffer[self.index-1]);
                    }
                }
                self.index += 1;
                return Some(self.buffer[self.index-1]);
            }
            self.index += 1;
        }
        return None;
    }

    pub fn parse_comment(&mut self) {
        let mut escaping = false;
        while self.index < self.len {
            match self.buffer[self.index] {
                b'*' => escaping = true,
                b'/' => if escaping == true {
                    self.index += 1;
                    return;
                },
                _ => escaping = false
            }
            self.index += 1;
        }
        self.create_error("No end to block comment!".to_string());
    }

    pub fn loc(&self) -> (u32, u32) {
        return (self.line, (self.index - self.line_index) as u32);
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

    pub fn create_error(&mut self, error: String) {
        self.errors.push(ParseError::new(self.line, (self.index-self.line_index) as u64,
                               get_line(self.buffer, self.line_index), error));
        self.skip_line();
    }

    pub fn skip_line(&mut self) {
        let line = self.line.clone();
        while line == self.line {
            match self.next_included() {
                Some(_) => {},
                None => {
                    break
                }
            }
        }
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

#[derive(Clone)]
pub struct ParseError {
    pub row: u32,
    pub column: u64,
    pub line: String,
    pub error: String
}

impl ParseError {
    pub fn new(row: u32, column: u64, line: String, error: String) -> Self {
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
        return write!(f, "Error at line \"{}\" ({}:{}): {}", self.line, self.row, self.column, self.error);
    }
}

pub fn parse(type_manager: &mut dyn TypeResolver,
                 name: &String, input: String) -> Result<(), Vec<ParseError>> {
    let mut parsing = ParseInfo::new(input.as_bytes());

    parse_top_elements(type_manager, name, &mut parsing);

    if !parsing.errors.is_empty() {
        return Err(parsing.errors);
    }
    return Ok(());
}