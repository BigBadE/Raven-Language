use syntax::code::Effects;
use syntax::ParsingError;

use crate::parser::code_parser::{parse_line, ParseState};
use crate::ParserUtils;
use data::tokens::{CodeErrorToken, TokenTypes};

/// Parses an operator effect naively, leaving a majority of the work for the checker
pub fn parse_operator(
    last: Option<Effects>,
    parser_utils: &mut ParserUtils,
    state: &ParseState,
) -> Result<Effects, ParsingError> {
    let mut operation = String::default();
    let mut effects = Vec::default();

    if let Some(effect) = last {
        operation += "{}";
        effects.push(effect);
    }

    parser_utils.index -= 1;
    while let Some(token) = parser_utils.tokens.get(parser_utils.index) {
        if token.token_type == TokenTypes::Operator
            || token.token_type == TokenTypes::Equals
            || token.token_type == TokenTypes::Period
        {
            operation += token.to_string(parser_utils.buffer).as_str();
        } else {
            break;
        }
        parser_utils.index += 1;
    }

    let mut first_element_token =
        CodeErrorToken::new(parser_utils.tokens[parser_utils.index].clone(), parser_utils.file.clone());
    let (mut index, mut tokens) = (parser_utils.index.clone(), parser_utils.tokens.len());
    let mut right = match parse_line(
        parser_utils,
        match state {
            ParseState::ControlVariable | ParseState::ControlOperator => ParseState::ControlOperator,
            _ => ParseState::InOperator,
        },
    ) {
        Ok(inner) => inner.map(|inner| inner.effect),
        Err(_) => None,
    };
    first_element_token.change_token_end(&parser_utils.tokens[parser_utils.index]);

    if right.is_some() {
        while parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type == TokenTypes::ArgumentEnd {
            (index, tokens) = (parser_utils.index.clone(), parser_utils.tokens.len());
            let mut next_element_token =
                CodeErrorToken::new(parser_utils.tokens[parser_utils.index].clone(), parser_utils.file.clone());
            let next = parse_line(parser_utils, ParseState::InOperator)?.map(|inner| inner.effect);
            next_element_token.change_token_end(&parser_utils.tokens[parser_utils.index]);
            if let Some(next_element) = next {
                if matches!(next_element, Effects::NOP) {
                    break;
                }
                right = match right.unwrap() {
                    Effects::CreateArray(mut inner) => {
                        inner.push((next_element, next_element_token));
                        Some(Effects::CreateArray(inner))
                    }
                    first_element => Some(Effects::CreateArray(vec![
                        (first_element, first_element_token.clone()),
                        (next_element, next_element_token),
                    ])),
                };
            } else {
                break;
            }
        }

        if let Some(inner) = &right {
            if matches!(*inner, Effects::NOP) {
                parser_utils.index = index;
                parser_utils.tokens.truncate(tokens);
                return Ok(Effects::Operation(
                    operation,
                    effects,
                    CodeErrorToken::new(parser_utils.tokens[parser_utils.index].clone(), parser_utils.file.clone()),
                ));
            } else {
                operation += "{}";
            }
        }
    } else {
        parser_utils.index = index;
        parser_utils.tokens.truncate(tokens);

        let mut last_token;
        loop {
            last_token = parser_utils.tokens.get(parser_utils.index).unwrap();
            if last_token.token_type == TokenTypes::Operator {
                operation += last_token.to_string(parser_utils.buffer).as_str();
            } else {
                break;
            }
            parser_utils.index += 1;
        }
    }

    if let Some(found) = right {
        effects.push(found);
    }

    let mut last = parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type.clone();
    while TokenTypes::BlockStart == last
        || TokenTypes::LineEnd == last
        || TokenTypes::BlockEnd == last
        || TokenTypes::ArgumentEnd == last
        || TokenTypes::ParenClose == last
    {
        parser_utils.index -= 1;
        last.clone_from(&parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type);
    }

    return Ok(Effects::Operation(
        operation,
        effects,
        CodeErrorToken::new(parser_utils.tokens[parser_utils.index].clone(), parser_utils.file.clone()),
    ));
}
