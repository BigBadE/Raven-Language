use syntax::code::Effects;
use syntax::ParsingError;

use crate::parser::code_parser::{parse_line, ParseState};
use crate::{ParserUtils, TokenTypes};

pub fn parse_operator(last: Option<Effects>, parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    if parser_utils.file == "build" {
        println!("Start parsing");
    }
    let mut operation = String::new();
    let mut effects = Vec::new();

    if let Some(effect) = last {
        operation += "{}";
        effects.push(effect);
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

    let (index, tokens) = (parser_utils.index.clone(), parser_utils.tokens.len());
    let mut right = match parse_line(parser_utils, ParseState::InOperator) {
        Ok(inner) => inner.map(|inner| inner.effect),
        Err(_) => None
    };
    if right.is_some() {
        while parser_utils.tokens.get(parser_utils.index-1).unwrap().token_type == TokenTypes::ArgumentEnd {
            let next = parse_line(parser_utils, ParseState::InOperator)?.map(|inner| inner.effect);
            if let Some(found) = next {
                if let Effects::NOP() = &found {
                    break
                }
                right = match right.unwrap() {
                    Effects::CreateArray(mut inner) => {
                        inner.push(found);
                        Some(Effects::CreateArray(inner))
                    },
                    other => Some(Effects::CreateArray(vec!(other, found)))
                };
            } else {
                break
            }
        }

        if let Some(inner) = &right {
            if let Effects::NOP() = inner {
                return Ok(Effects::Operation(operation, effects));
            } else {
                operation += "{}";
            }
        }
    } else {
        parser_utils.index = index;
        parser_utils.tokens.truncate(tokens);
    }

    let mut last_token;
    loop {
        last_token = parser_utils.tokens.get(parser_utils.index).unwrap();
        if last_token.token_type == TokenTypes::Operator {
            operation += last_token.to_string(parser_utils.buffer).as_str();
        } else {
            break
        }
        parser_utils.index += 1;
    }

    if let Some(found) = right {
        effects.push(found);
    }

    if TokenTypes::LineEnd == parser_utils.tokens.get(parser_utils.index-1).unwrap().token_type {
        parser_utils.index -= 1;
    }

    if parser_utils.file == "build" {
        println!("End parsing: {}, {:?}", operation, effects);
    }
    return Ok(Effects::Operation(operation, effects));
}