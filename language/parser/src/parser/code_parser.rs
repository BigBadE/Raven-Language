use std::future::Future;
use std::pin::Pin;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use syntax::types::Types;
use crate::parser::control_parser::parse_for;
use crate::parser::util::{add_generics, ParserUtils};
use crate::tokens::tokens::{Token, TokenTypes};

pub type ParsingFuture<T> = Pin<Box<dyn Future<Output=Result<T, ParsingError>> + Send>>;

pub fn parse_code(parser_utils: &mut ParserUtils) -> impl Future<Output=Result<CodeBody, ParsingError>> {
    let mut lines = Vec::new();
    while let Some((expression, effect)) = parse_line(parser_utils, false, false) {
        lines.push(get_line(effect, expression));
        if parser_utils.tokens.get(parser_utils.index-1).unwrap().token_type == TokenTypes::BlockEnd {
            break
        }
    }
    parser_utils.imports.last_id += 1;
    return create_body(parser_utils.imports.last_id - 1, lines);
}

pub fn parse_line(parser_utils: &mut ParserUtils, break_at_body: bool, deep: bool)
                  -> Option<(ExpressionType, ParsingFuture<Effects>)> {
    println!("Calling!");
    let mut effect = None;
    let mut expression_type = ExpressionType::Line;
    loop {
        //TODO add rest
        let token = parser_utils.tokens.get(parser_utils.index).unwrap(); parser_utils.index += 1;
        println!("Token: {:?}", token.token_type);
        match token.token_type {
            TokenTypes::ParenOpen => {
                println!("Nesting");
                if let Some((_, in_effect)) = parse_line(parser_utils, break_at_body, true) {
                    effect = Some(in_effect);
                } else {
                    effect = None;
                }
                println!("Nest done");
            }
            TokenTypes::Float => {
                effect = Some(constant_effect(Effects::Float(token.to_string(parser_utils.buffer).parse().unwrap())))
            }
            TokenTypes::Integer => {
                effect = Some(constant_effect(Effects::Int(token.to_string(parser_utils.buffer).parse().unwrap())))
            }
            TokenTypes::LineEnd | TokenTypes::ParenClose | TokenTypes::CodeEnd | TokenTypes::BlockEnd => {
                println!("Breaking");
                break;
            }
            TokenTypes::Variable => {
                effect = Some(constant_effect(Effects::LoadVariable(token.to_string(parser_utils.buffer))))
            }
            TokenTypes::Return => expression_type = ExpressionType::Return,
            TokenTypes::New => effect = Some(parse_new(parser_utils)),
            TokenTypes::BlockStart => if break_at_body {
                println!("Breaking!");
                break;
            } else {
                effect = Some(Box::pin(body_effect(parse_code(parser_utils))))
            },
            TokenTypes::For => return Some((expression_type, parse_for(parser_utils))),
            //TODO
            TokenTypes::Operator => {}
            TokenTypes::ArgumentEnd => if !deep {
                break;
            },
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
    return Some((expression_type, effect.unwrap_or(constant_effect(Effects::NOP()))));
}

async fn body_effect(body: impl Future<Output=Result<CodeBody, ParsingError>>) -> Result<Effects, ParsingError> {
    return Ok(Effects::CodeBody(body.await?));
}

fn constant_effect(effect: Effects) -> ParsingFuture<Effects> {
    return Box::pin(constant_effect_inner(Ok(effect)));
}

async fn constant_effect_inner(effect: Result<Effects, ParsingError>) -> Result<Effects, ParsingError> {
    return effect;
}

fn parse_new(parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
    let mut types: Option<ParsingFuture<Types>> = None;
    let values;

    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap(); parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                types = Some(Box::pin(parser_utils
                    .get_struct(token, token.to_string(parser_utils.buffer))))
            }
            //Handle making new structs with generics.
            TokenTypes::Operator => {
                types = Some(add_generics(types.unwrap(), parser_utils));
            }
            TokenTypes::BlockStart => {
                values = parse_new_args(parser_utils);
                break;
            }
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return Box::pin(create_effect(Box::pin(types.unwrap()), values));
}

fn parse_new_args(parser_utils: &mut ParserUtils) -> Vec<(usize, ParsingFuture<Effects>)> {
    let mut values = Vec::new();
    let mut name = String::new();
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap(); parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => name = token.to_string(parser_utils.buffer),
            TokenTypes::Colon | TokenTypes::ArgumentEnd => {
                let effect = if let TokenTypes::Colon = token.token_type {
                    let token = token.clone();
                    parse_line(parser_utils, false, false).unwrap_or((ExpressionType::Line,
                                                               Box::pin(expect_effect(token)))).1
                } else {
                    constant_effect(Effects::LoadVariable(name))
                };
                name = String::new();
                values.push((0, effect));
            }
            TokenTypes::BlockEnd => break,
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return values;
}

async fn expect_effect(token: Token) -> Result<Effects, ParsingError> {
    return Err(token.make_error("Expected something, found void".to_string()));
}

async fn create_effect(types: ParsingFuture<Types>, inputs: Vec<(usize, ParsingFuture<Effects>)>)
                       -> Result<Effects, ParsingError> {
    let mut final_inputs = Vec::new();
    for input in inputs {
        final_inputs.push((input.0, input.1.await?));
    }
    return Ok(Effects::CreateStruct(types.await?, final_inputs));
}

pub async fn get_line(effect: ParsingFuture<Effects>, expression_type: ExpressionType)
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