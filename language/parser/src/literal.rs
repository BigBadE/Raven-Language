use ast::code::{Effects, NumberEffect};
use crate::parser::ParseInfo;

pub fn parse_string(parsing: &mut ParseInfo) -> Option<String> {
    let mut output = String::new();
    let mut escape = false;
    loop {
        if parsing.len == parsing.index {
            parsing.create_error("Missing end to string!".to_string());
            return None;
        }
        match parsing.buffer[parsing.index] {
            b'\\' => if escape {
                escape = false;
                output.push('\\');
            } else {
                escape = true;
            }
            b'"' => if !escape {
                break
            },
            found => {
                escape = false;
                output.push(found as char);
            }
        }
        parsing.index += 1;
    }

    return Some(output)
}

pub fn parse_number<'a>(parsing: &mut ParseInfo) -> Option<Effects> {
    let start = parsing.index;
    let mut float = false;
    loop {
        if parsing.len == parsing.index {
            break
        }
        //Negatives are handled separate
        match parsing.buffer[parsing.index] {
            b'0'..=b'9' => {},
            b'.' => float = true,
            _ => break
        }
        parsing.index += 1;
    }
    return if parsing.index == start || parsing.index == start + 1 && float {
        None
    } else if float {
        Some(Effects::FloatEffect(Box::new(NumberEffect::new(
            String::from_utf8_lossy(&parsing.buffer[start..parsing.index]).parse::<f64>().unwrap()))))
    } else {
        Some(Effects::IntegerEffect(Box::new(NumberEffect::new(
            String::from_utf8_lossy(&parsing.buffer[start..parsing.index]).parse::<i64>().unwrap()))))
    }
}

pub fn parse_ident(parsing: &mut ParseInfo) -> String {
    let start = parsing.index;
    loop {
        parsing.index += 1;
        if parsing.index == parsing.len {
            break
        }

        match parsing.buffer[parsing.index] {
            b'a'..=b'z' => {},
            b'A'..=b'Z' => {},
            _ => break
        }
    }

    return String::from_utf8_lossy(&parsing.buffer[start..parsing.index]).to_string();
}

pub fn parse_with_references(parsing: &mut ParseInfo) -> String {
    let start = parsing.index;
    loop {
        parsing.index += 1;
        if parsing.index == parsing.len {
            break
        }

        match parsing.buffer[parsing.index] {
            b'a'..=b'z' => {},
            b'A'..=b'Z' => {},
            b':' => {},
            _ => break
        }
    }

    return String::from_utf8_lossy(&parsing.buffer[start..parsing.index]).to_string();
}