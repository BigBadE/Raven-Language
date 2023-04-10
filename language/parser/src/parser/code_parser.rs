use std::future::Future;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use crate::parser::util::ParserUtils;
use crate::tokens::tokens::TokenTypes;

pub fn parse_code(parser_utils: &mut ParserUtils) -> impl Future<Output=Result<CodeBody, ParsingError>> {
    let mut lines = Vec::new();
    while let Some((expression, effect)) = parse_line(parser_utils) {
        lines.push(get_line(effect, expression));
    }
    parser_utils.imports.last_id += 1;
    return create_body(parser_utils.imports.last_id-1, lines);
}

pub fn parse_line(parser_utils: &mut ParserUtils) -> Option<(ExpressionType, Effects)> {
    let mut effect = None;
    let expression_type = ExpressionType::Line;
    loop {
        //TODO add rest
        let token = parser_utils.tokens.remove(0);
        match token.token_type {
            TokenTypes::ParenOpen => {
                if let Some((_, in_effect)) = parse_line(parser_utils) {
                    effect = Some(in_effect);
                } else {
                    effect = None;
                }
            },
            TokenTypes::Float => {
                effect = Some(Effects::Float(token.to_string(parser_utils.buffer).parse().unwrap()))
            },
            TokenTypes::Integer => {
                effect = Some(Effects::Int(token.to_string(parser_utils.buffer).parse().unwrap()))
            },
            TokenTypes::LineEnd | TokenTypes::ParenClose => break,
            _ => panic!("How'd you get here?")
        }
    }
    return match effect {
        Some(effect) => Some((expression_type, effect)),
        None => Some((expression_type, Effects::NOP()))
    };
}

pub async fn get_line(effect: Effects, expression_type: ExpressionType) -> Expression {
    return Expression::new(expression_type, effect);
}

pub async fn create_body(id: u32, lines: Vec<impl Future<Output=Expression>>) -> Result<CodeBody, ParsingError> {
    let mut body = Vec::new();
    for line in lines {
        body.push(line.await);
    }
    return Ok(CodeBody::new(body, id.to_string()));
}