use syntax::code::Effects;
use syntax::ParsingError;

use crate::parser::code_parser::parse_line;
use crate::{ParserUtils, TokenTypes};

pub fn parse_operator(last: Option<Effects>, parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let mut operation = String::new();
    if last.is_some() {
        operation += "{}";
    }

    parser_utils.index -= 1;
    while let Some(token) = parser_utils.tokens.get(parser_utils.index) {
        if token.token_type == TokenTypes::Operator || token.token_type == TokenTypes::Equals {
            operation += token.to_string(parser_utils.buffer).as_str();
        } else {
            break
        }
        parser_utils.index += 1;
    }

    let right = parse_line(parser_utils, true, false)?;
    if right.is_some() {
        operation += "{}";
    }

    return create_operator(operation, last, right.map(|inner| inner.effect));
}

fn create_operator(name: String, lhs: Option<Effects>,
                         rhs: Option<Effects>) -> Result<Effects, ParsingError> {
    let mut arguments = Vec::new();

    if let Some(found) = lhs {
        arguments.push(found);
    }
    if let Some(found) = rhs {
        arguments.push(found);
    }

    return Ok(Effects::Operation(name, arguments));
}