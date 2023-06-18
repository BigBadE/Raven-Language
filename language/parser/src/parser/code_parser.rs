use std::future::Future;
use std::sync::{Arc, Mutex};
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::{ParsingError, ParsingFuture};
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::syntax::Syntax;
use crate::parser::control_parser::{parse_for, parse_if};
use crate::parser::operator_parser::parse_operator;
use crate::parser::util::{add_generics, ParserUtils};
use crate::tokens::tokens::{Token, TokenTypes};

pub fn parse_code(parser_utils: &mut ParserUtils) -> (ExpressionType, ParsingFuture<CodeBody>) {
    let mut lines = Vec::new();
    let mut types = ExpressionType::Line;
    while let Some((expression, effect)) =
        parse_line(parser_utils, false, false) {
        if expression != ExpressionType::Line {
            types = expression;
        }
        lines.push(get_line(effect, expression));
    }
    parser_utils.imports.last_id += 1;
    return (types, Box::pin(create_body(parser_utils.imports.last_id - 1, lines)));
}

pub fn parse_line(parser_utils: &mut ParserUtils, break_at_body: bool, deep: bool)
                  -> Option<(ExpressionType, ParsingFuture<Effects>)> {
    let mut effect: Option<ParsingFuture<Effects>> = None;
    let mut expression_type = ExpressionType::Line;
    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap().clone();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::ParenOpen => {
                let last = parser_utils.tokens.get(parser_utils.index - 2).unwrap().clone();
                match last.token_type {
                    TokenTypes::Variable | TokenTypes::CallingType => {
                        let mut effects = Vec::new();
                        parser_utils.index += 1;
                        if parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type != TokenTypes::ParenClose {
                            while let Some((_, effect)) = parse_line(parser_utils, false, false) {
                                effects.push(effect);
                                if parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type == TokenTypes::ArgumentEnd {} else {
                                    break;
                                }
                            }
                            parser_utils.index -= 1;
                        }

                        let name = last.to_string(parser_utils.buffer);
                        effect = Some(Box::pin(method_call(effect, name.clone(), effects)))
                    }
                    _ => if let Some((_, in_effect)) = parse_line(parser_utils, break_at_body, true) {
                        effect = Some(in_effect);
                    } else {
                        effect = None;
                    }
                }
            }
            TokenTypes::Float => {
                effect = Some(constant_effect(Effects::Float(token.to_string(parser_utils.buffer).parse().unwrap())))
            }
            TokenTypes::Integer => {
                effect = Some(constant_effect(Effects::Int(token.to_string(parser_utils.buffer).parse().unwrap())))
            },
            TokenTypes::True => {
                effect = Some(constant_effect(Effects::Bool(true)))
            },
            TokenTypes::False => {
                effect = Some(constant_effect(Effects::Bool(false)))
            },
            TokenTypes::LineEnd | TokenTypes::ParenClose => break,
            TokenTypes::CodeEnd | TokenTypes::BlockEnd => return None,
            TokenTypes::Variable =>
                if let TokenTypes::ParenOpen = parser_utils.tokens.get(parser_utils.index).unwrap().token_type {} else {
                    effect = Some(constant_effect(
                        Effects::LoadVariable(token.to_string(parser_utils.buffer))))
                },
            TokenTypes::Return => expression_type = ExpressionType::Return,
            TokenTypes::New => effect = Some(parse_new(parser_utils)),
            TokenTypes::BlockStart => if break_at_body {
                break;
            } else {
                let (returning, body) = parse_code(parser_utils);
                if expression_type == ExpressionType::Line {
                    expression_type = returning;
                }
                effect = Some(Box::pin(body_effect(body)))
            },
            TokenTypes::Let => return Some((expression_type, parse_let(parser_utils))),
            TokenTypes::If => {
                let (returning, body) = parse_if(parser_utils);
                if expression_type == ExpressionType::Line {
                    expression_type = returning;
                }
                effect = Some(body)
            },
            TokenTypes::For => return Some((expression_type, parse_for(parser_utils))),
            TokenTypes::Equals => {
                let other = parser_utils.tokens.get(parser_utils.index).unwrap().token_type.clone();
                if effect.is_some() && other != TokenTypes::Operator && other != TokenTypes::Equals {
                    let error = token.make_error(parser_utils.file.clone(), "Tried to assign a void value!".to_string());
                    let value = parse_line(parser_utils, false, false);
                    if let Some(value) = value {
                        effect = Some(Box::pin(create_assign(effect.unwrap(), value.1)));
                    } else {
                        effect = Some(constant_error(error));
                    }
                } else {
                    return Some((expression_type, parse_operator(effect, parser_utils)));
                }
            }
            TokenTypes::Operator => {
                return Some((expression_type, parse_operator(effect, parser_utils)))
            },
            TokenTypes::ArgumentEnd => if !deep {
                break;
            },
            TokenTypes::CallingType =>
                if let TokenTypes::ParenOpen = parser_utils.tokens.get(parser_utils.index).unwrap().token_type {
                    //Ignored, ParenOpen handles this
                } else {
                    effect = Some(Box::pin(load_effect(effect.unwrap(), token.to_string(parser_utils.buffer))))
                },
            TokenTypes::EOF => {
                parser_utils.index -= 1;
                break;
            }
            TokenTypes::Else => return Some((expression_type, constant_error(token.make_error(parser_utils.file.clone(),
                                                                                              "Unexpected Else!".to_string())))),
            TokenTypes::Period | TokenTypes::Comment => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }
    return Some((expression_type, effect.unwrap_or(constant_effect(Effects::NOP()))));
}

async fn method_call(calling: Option<ParsingFuture<Effects>>, name: String,
                     inner: Vec<ParsingFuture<Effects>>) -> Result<Effects, ParsingError> {
    let mut output = Vec::new();
    let calling = match calling {
        Some(found) => Some(Box::new(found.await?)),
        None => None
    };

    for possible in inner {
        output.push(possible.await?);
    }

    return Ok(Effects::MethodCall(calling, name, output));
}

async fn load_effect(loading: ParsingFuture<Effects>, name: String) -> Result<Effects, ParsingError> {
    return Ok(Effects::Load(Box::new(loading.await?), name));
}

async fn body_effect(body: impl Future<Output=Result<CodeBody, ParsingError>>) -> Result<Effects, ParsingError> {
    return Ok(Effects::CodeBody(body.await?));
}

fn constant_effect(effect: Effects) -> ParsingFuture<Effects> {
    return Box::pin(constant_effect_inner(Ok(effect)));
}

fn constant_error(error: ParsingError) -> ParsingFuture<Effects> {
    return Box::pin(constant_effect_inner(Err(error)));
}

async fn constant_effect_inner(effect: Result<Effects, ParsingError>) -> Result<Effects, ParsingError> {
    return effect;
}

async fn create_assign(last: ParsingFuture<Effects>, effect: ParsingFuture<Effects>) -> Result<Effects, ParsingError> {
    return Ok(Effects::Set(Box::new(last.await?), Box::new(effect.await?)));
}

fn parse_let(parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
    let name;
    {
        let next = parser_utils.tokens.get(parser_utils.index).unwrap();
        if let TokenTypes::Variable = next.token_type {
            name = next.to_string(parser_utils.buffer);
        } else {
            return constant_error(next.make_error(parser_utils.file.clone(), "Unexpected token, expected variable name!".to_string()));
        }

        if let TokenTypes::Equals = parser_utils.tokens.get(parser_utils.index + 1).unwrap().token_type {} else {
            return constant_error(next.make_error(parser_utils.file.clone(), format!("Unexpected {:?}, expected equals!", next)));
        }
        parser_utils.index += 2;
    }

    return match parse_line(parser_utils, false, false) {
        Some(line) => Box::pin(create_let(name, line.1)),
        None => constant_error(parser_utils.tokens.get(parser_utils.index).unwrap()
            .make_error(parser_utils.file.clone(), "Expected value, found void!".to_string()))
    };
}

async fn create_let(name: String, value: ParsingFuture<Effects>) -> Result<Effects, ParsingError> {
    let value = value.await?;
    return Ok(Effects::CreateVariable(name, Box::new(value)));
}

fn parse_new(parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
    let mut types: Option<UnparsedType> = None;

    let values;

    loop {
        let token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => {
                types = Some(UnparsedType::Basic(token.to_string(parser_utils.buffer)))
            }
            //Handle making new structs with generics.
            TokenTypes::Operator => {
                types = Some(add_generics(types.unwrap().to_string(), parser_utils).0);
            }
            TokenTypes::BlockStart => {
                values = parse_new_args(parser_utils);
                break;
            }
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return Box::pin(create_effect(parser_utils.syntax.clone(),
                                  parser_utils.tokens.get(parser_utils.index).unwrap().clone(),
                                  parser_utils.file.clone(),
                                  parser_utils.imports.boxed_clone(),
                                  types.unwrap(), values));
}

fn parse_new_args(parser_utils: &mut ParserUtils) -> Vec<(String, ParsingFuture<Effects>)> {
    let mut values = Vec::new();
    let mut name = String::new();
    loop {
        let token: &Token = parser_utils.tokens.get(parser_utils.index).unwrap();
        parser_utils.index += 1;
        match token.token_type {
            TokenTypes::Variable => name = token.to_string(parser_utils.buffer),
            TokenTypes::Colon | TokenTypes::ArgumentEnd => {
                let effect = if let TokenTypes::Colon = token.token_type {
                    let token = token.clone();
                    parse_line(parser_utils, false, false)
                        .unwrap_or((ExpressionType::Line,
                                    Box::pin(expect_effect(parser_utils.file.clone(), token)))).1
                } else {
                    constant_effect(Effects::LoadVariable(name.clone()))
                };
                values.push((name, effect));
                name = String::new();
                if parser_utils.tokens.get(parser_utils.index - 1).unwrap().token_type == TokenTypes::BlockEnd {
                    break;
                }
            }
            TokenTypes::BlockEnd => break,
            TokenTypes::InvalidCharacters => {}
            _ => panic!("How'd you get here? {:?}", token.token_type)
        }
    }

    return values;
}

async fn expect_effect(file: String, token: Token) -> Result<Effects, ParsingError> {
    return Err(token.make_error(file, "Expected something, found void".to_string()));
}

async fn create_effect(syntax: Arc<Mutex<Syntax>>, token: Token, file: String, resolver: Box<dyn NameResolver>,
                       types: UnparsedType, inputs: Vec<(String, ParsingFuture<Effects>)>)
                       -> Result<Effects, ParsingError> {
    let types = Syntax::parse_type(syntax, token.make_error(file.clone(), format!("Unknown type {}", types)), resolver, types).await?;
    let fields = types.get_fields();
    let mut final_inputs = Vec::new();
    for input in inputs {
        let mut i = 0;
        for field in fields {
            if unsafe { very_bad_function(field) }.await_finish().await?.field.name == input.0 {
                final_inputs.push((i, input.1.await?));
                break
            }
            i += 1;
        }
        if i == fields.len() {
            return Err(token.make_error(file, format!("No field named {}!", input.0)));
        }
    }
    return Ok(Effects::CreateStruct(types, final_inputs));
}

//This turns an immutable reference mutable.
//Why? Because it's a pain to trace the reference for testing back to the Arc.
unsafe fn very_bad_function<T>(reference: &T) -> &mut T {
    let const_ptr = reference as *const T;
    let mut_ptr = const_ptr as *mut T;
    &mut *mut_ptr
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