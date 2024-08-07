use syntax::errors::ParsingError;
use syntax::program::code::{EffectType, Effects};

use crate::parser::code_parser::{parse_line, ParseState};
use crate::ParserUtils;
use data::tokens::{Span, TokenTypes};

/// Parses an operator effect naively, leaving a majority of the work for the checker
pub fn parse_operator(
    last: Option<Effects>,
    parser_utils: &mut ParserUtils,
    state: &ParseState,
) -> Result<Effects, ParsingError> {
    // In the operation syntax, {} is used to represent a value
    let mut operation = String::default();
    let mut effects = Vec::default();

    // If there's an effect before it, it must start with something like {}+{} instead of an effect like !{}
    if let Some(effect) = last {
        operation += "{}";
        effects.push(effect);
    }

    // Backtrack before the operator and add all of the operator to the operation string
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

    // Parse the right effect of the operator
    let mut first_element_token = Span::new(parser_utils.file, parser_utils.index);
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
    first_element_token.extend_span(parser_utils.index);

    // If it's a list effect, get all the elements
    if right.is_some() {
        while parser_utils.tokens.get(parser_utils.index).unwrap().token_type == TokenTypes::ArgumentEnd {
            (index, tokens) = (parser_utils.index.clone(), parser_utils.tokens.len());
            let mut next_element_token = Span::new(parser_utils.file, parser_utils.index);
            let next = parse_line(parser_utils, ParseState::InOperator)?.map(|inner| inner.effect);
            next_element_token.extend_span(parser_utils.index);
            if let Some(next_element) = next {
                parser_utils.index += 1;
                right = match right.unwrap().types {
                    EffectType::CreateArray(mut inner) => {
                        inner.push(next_element);
                        Some(Effects::new(next_element_token, EffectType::CreateArray(inner)))
                    }
                    first_element => Some(Effects::new(
                        next_element_token,
                        EffectType::CreateArray(vec![
                            Effects::new(first_element_token.clone(), first_element),
                            next_element,
                        ]),
                    )),
                };
            } else {
                break;
            }
        }

        if right.is_none() {
            parser_utils.index = index;
            parser_utils.tokens.truncate(tokens);
            return Ok(Effects::new(
                Span::new(parser_utils.file, parser_utils.index),
                EffectType::Operation(operation, effects),
            ));
        } else {
            operation += "{}";
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

    return Ok(Effects {
        types: EffectType::Operation(operation, effects),
        span: Span::new(parser_utils.file, parser_utils.index),
    });
}
