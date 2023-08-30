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

    let mut right = parse_line(parser_utils, true, false)?.map(|inner| inner.effect);
    if right.is_some() {
        while parser_utils.tokens.get(parser_utils.index-1).unwrap().token_type == TokenTypes::ArgumentEnd {
            println!("Found arg end!");
            let next = parse_line(parser_utils, true, false)?.map(|inner| inner.effect);
            if let Some(found) = next {
                right = match right.unwrap() {
                    Effects::CreateArray(mut inner) => {
                        inner.push(found);
                        Some(Effects::CreateArray(inner))
                    },
                    other => Some(Effects::CreateArray(vec!(other, found)))
                };
                println!("Added!");
            } else {
                println!("Nope");
                break
            }
        }

        operation += "{}";
    }

    let mut last_token;
    loop {
        println!("Tokens: {:?}", parser_utils.tokens[parser_utils.index-5..parser_utils.index]
            .iter().map(|token| &token.token_type).collect::<Vec<_>>());
        last_token = parser_utils.tokens.get(parser_utils.index-2).unwrap();
        println!("Next: {:?}", last_token.token_type);
        if last_token.token_type == TokenTypes::Operator {
            operation += last_token.to_string(parser_utils.buffer).as_str();
        } else {
            break
        }
        parser_utils.index += 1;
    }
    return create_operator(operation, last, right);
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