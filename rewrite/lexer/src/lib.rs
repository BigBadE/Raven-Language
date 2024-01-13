use std::collections::HashMap;

pub static NUMBER: u64 = 0;
pub static STRING: u64 = 1;
pub static VARIABLE: u64 = 2;
pub static UNKNOWN: u64 = 3;

pub fn lex(keywords: &HashMap<u64, &'static str>, file: &[u8]) -> Vec<Token> {
    let mut output = vec![];
    let mut index = 0;
    'outer: while index < file.len() {
        for (token_id, name) in keywords {
            if let Some(found) = matches(file, &mut index, name) {
                output.push(Token { id: *token_id, span: Span::new(found, index) });
                continue 'outer;
            }
        }

        if let Some(found) = is_number(file, &mut index) {
            output.push(Token { id: NUMBER, span: Span::new(found, index) });
        } else if let Some(found) = is_string(file, &mut index) {
            output.push(Token { id: STRING, span: Span::new(found, index) });
        } else if let Some(found) = is_variable(file, &mut index) {
            output.push(Token { id: VARIABLE, span: Span::new(found, index) });
        } else {
            output.push(Token { id: UNKNOWN, span: Span::new(index, index + 1) });
            index += 1;
        }
    }
    return output;
}

pub fn matches(file: &[u8], index: &mut usize, matching: &str) -> Option<usize> {
    remove_whitespace(file, index);
    if *index + matching.len() >= file.len() {
        return None;
    }

    let temp = *index;
    for testing in matching.as_bytes() {
        if file[*index] != *testing {
            *index = temp;
            return None;
        }
        *index += 1;
    }
    return Some(temp);
}

pub fn is_variable(file: &[u8], index: &mut usize) -> Option<usize> {
    remove_whitespace(file, index);

    let temp = *index;
    while *index < file.len() && is_valid_variable(file[*index] as char) {
        *index += 1;
    }

    if temp == *index {
        return None;
    }
    return Some(temp);
}

fn is_valid_variable(found: char) -> bool {
    return found.is_alphabetic() || found == '_' || found == '-';
}

pub fn is_string(file: &[u8], index: &mut usize) -> Option<usize> {
    remove_whitespace(file, index);

    let temp = *index;
    if file[*index] != b'"' {
        return None;
    }
    *index += 1;
    let mut special = false;
    while *index < file.len() {
        if special {
            special = false;
            *index += 1;
            continue;
        }
        match file[*index] {
            b'\\' => special = true,
            b'"' => break,
            _ => {}
        }
        *index += 1;
    }

    return Some(temp);
}

pub fn is_number(file: &[u8], index: &mut usize) -> Option<usize> {
    remove_whitespace(file, index);

    let temp = *index;
    let mut has_period = false;
    while *index < file.len() {
        if (file[*index] as char).is_numeric() || (*index == temp && file[*index] == b'-') {
            // Skip numbers and the minus
        } else if !has_period && file[*index] == b'.' {
            has_period = true;
        } else {
            break;
        }
        *index += 1;
    }

    if temp == *index {
        return None;
    }

    return Some(temp);
}

pub fn remove_whitespace(file: &[u8], index: &mut usize) {
    while (file[*index] as char).is_whitespace() {
        *index += 1;
    }
}

pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        return Span { start, end };
    }
}

pub struct Token {
    pub id: u64,
    span: Span,
}
