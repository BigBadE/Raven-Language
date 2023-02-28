use ast::r#struct::TypeMembers;
use ast::TopElement;

pub struct ParseInfo<'a> {
    state: State,
    buffer: &'a [u8],
    index: usize,
    len: usize,
    line: u64
}

impl<'a> ParseInfo<'a> {
    pub fn new(buffer: &'a String) -> Self {
        return Self {
            state: State::Starting,
            len: buffer.len() as usize,
            buffer: buffer.as_bytes(),
            index: 0,
            line: 0
        }
    }

    pub fn next_included(&mut self) {
        while self.index < self.len && self.is_whitespace(self.buffer[self.index]) {
            self.index += 1
        }
    }

    fn whitespace_next(&mut self, char: u8) -> bool {
        return if char == b' ' || char == b'\t' || char == b'\r' {
            true
        } else if char == b'\n' {
            self.line += 1;
            true
        } else {
            false
        }
    }
}

pub enum State {
    Starting,
}

pub fn parse(name: &String, input: String) -> Vec<TopElement> {
    let output = Vec::new();
    let parsing = ParseInfo::new(&input);
    let errors = Vec::new();

    return output;
}