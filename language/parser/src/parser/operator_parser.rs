use syntax::code::Effects;
use syntax::{ParsingError, ParsingFuture};

use crate::parser::code_parser::parse_line;
use crate::{ParserUtils, TokenTypes};

pub fn parse_operator(last: Option<ParsingFuture<Effects>>, parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
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

    let right = parse_line(parser_utils, false, false);
    if right.is_some() {
        operation += "{}";
    }

    return Box::pin(create_operator(operation, last, right.map(|found| found.1)));
}

async fn create_operator(name: String, lhs: Option<ParsingFuture<Effects>>,
                         rhs: Option<ParsingFuture<Effects>>) -> Result<Effects, ParsingError> {
    let mut arguments = Vec::new();

    if let Some(found) = lhs {
        arguments.push(found.await?);
    }
    if let Some(found) = rhs {
        arguments.push(found.await?);
    }

    return Ok(Effects::Operation(name, arguments));
}