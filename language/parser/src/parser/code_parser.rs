use std::future::Future;
use std::pin::Pin;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use syntax::types::Types;
use crate::parser::util::{add_generics, ParserUtils};
use crate::tokens::tokens::{Token, TokenTypes};

pub fn parse_code(parser_utils: &mut ParserUtils) -> impl Future<Output=Result<CodeBody, ParsingError>> {
    let mut lines = Vec::new();
    while let Some((expression, effect)) = parse_line(parser_utils) {
        lines.push(get_line(effect, expression));
    }
    parser_utils.imports.last_id += 1;
    return create_body(parser_utils.imports.last_id - 1, lines);
}

pub fn parse_line(parser_utils: &mut ParserUtils)
                  -> Option<(ExpressionType, Pin<Box<dyn Future<Output=Result<Effects, ParsingError>> + Send>>)> {
    let mut effect = None;
    let mut expression_type = ExpressionType::Line;
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
            }
            TokenTypes::Float => {
                effect = Some(constant_effect(Effects::Float(token.to_string(parser_utils.buffer).parse().unwrap())))
            }
            TokenTypes::Integer => {
                effect = Some(constant_effect(Effects::Int(token.to_string(parser_utils.buffer).parse().unwrap())))
            }
            TokenTypes::LineEnd | TokenTypes::ParenClose => break,
            TokenTypes::Variable => {
                effect = Some(constant_effect(Effects::LoadVariable(token.to_string(parser_utils.buffer))))
            }
            TokenTypes::Return => expression_type = ExpressionType::Return,
            TokenTypes::New => effect = Some(parse_new(parser_utils)),
            TokenTypes::BlockStart => effect = Some(Box::pin(body_effect(parse_code(parser_utils)))),
            TokenTypes::BlockEnd => break,
            //TODO
            TokenTypes::Operator => {},
            TokenTypes::CodeEnd | TokenTypes::ArgumentEnd => break,
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
    return Some((expression_type, effect.unwrap_or(constant_effect(Effects::NOP()))));
}

async fn body_effect(body: impl Future<Output=Result<CodeBody, ParsingError>>) -> Result<Effects, ParsingError> {
    return Ok(Effects::CodeBody(body.await?));
}

fn constant_effect(effect: Effects) -> Pin<Box<dyn Future<Output=Result<Effects, ParsingError>> + Send>> {
    return Box::pin(constant_effect_inner(Ok(effect)));
}

async fn constant_effect_inner(effect: Result<Effects, ParsingError>) -> Result<Effects, ParsingError> {
    return effect;
}

fn parse_new(parser_utils: &mut ParserUtils) -> Pin<Box<dyn Future<Output=Result<Effects, ParsingError>> + Send>> {
    let mut types: Option<Pin<Box<dyn Future<Output=Result<Types, ParsingError>> + Send>>> = None;
    let values;

    loop {
        let token = parser_utils.tokens.remove(0);
        match token.token_type {
            TokenTypes::Variable => {
                types = Some(Box::pin(parser_utils
                    .get_struct(token.clone(), token.to_string(parser_utils.buffer))))
            },
            //Handle making new structs with generics.
            TokenTypes::Operator => {
                types = Some(add_generics(types.unwrap(), parser_utils));
            }
            TokenTypes::BlockStart => {
                values = parse_new_args(parser_utils);
                break
            }
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return Box::pin(create_effect(Box::pin(types.unwrap()), values));
}

fn parse_new_args(parser_utils: &mut ParserUtils)
    -> Vec<(usize, Pin<Box<dyn Future<Output=Result<Effects, ParsingError>> + Send>>)> {
    let mut values = Vec::new();
    let mut name = String::new();
    loop {
        let token = parser_utils.tokens.remove(0);
        match token.token_type {
            TokenTypes::Variable => name = token.to_string(parser_utils.buffer),
            TokenTypes::Colon | TokenTypes::ArgumentEnd => {
                let effect = if let TokenTypes::Colon = token.token_type {
                    parse_line(parser_utils).unwrap_or((ExpressionType::Line,
                                                           Box::pin(expect_effect(token.clone())))).1
                } else {
                    constant_effect(Effects::LoadVariable(name))
                };
                name = String::new();
                values.push((0, effect));
            },
            TokenTypes::BlockEnd => break,
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return values;
}

async fn expect_effect(token: Token) -> Result<Effects, ParsingError> {
    return Err(token.make_error("Expected something, found void".to_string()))
}

async fn create_effect(types: Pin<Box<dyn Future<Output=Result<Types, ParsingError>> + Send>>,
    inputs: Vec<(usize, Pin<Box<dyn Future<Output=Result<Effects, ParsingError>> + Send>>)>)
    -> Result<Effects, ParsingError> {
    let mut final_inputs = Vec::new();
    for input in inputs {
        final_inputs.push((input.0, input.1.await?));
    }
    return Ok(Effects::CreateStruct(types.await?, final_inputs));
}

pub async fn get_line(effect: Pin<Box<dyn Future<Output=Result<Effects, ParsingError>> + Send>>, expression_type: ExpressionType)
    -> Result<Expression, ParsingError> {
    return Ok(Expression::new(expression_type, effect.await?));
}

pub async fn create_body(id: u32, lines: Vec<impl Future<Output=Result<Expression, ParsingError>>>)
    -> Result<CodeBody, ParsingError> {
    let mut body = Vec::new();
    for line in lines {
        body.push(line.await?);
    }
    return Ok(CodeBody::new(body, id.to_string()));
}