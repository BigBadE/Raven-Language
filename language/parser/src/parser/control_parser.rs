use std::future::Future;
use std::sync::{Arc, Mutex};
use syntax::async_util::NameResolver;

use async_recursion::async_recursion;
use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::{ParsingError, ParsingFuture};
use syntax::syntax::Syntax;
use crate::parser::code_parser::{parse_code, parse_line};

use crate::{ParserUtils, TokenTypes};

pub fn parse_if(parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
    let effect = parse_line(parser_utils, true, false);
    if effect.is_none() {
        return Box::pin(create_error(parser_utils.tokens.get(parser_utils.index).unwrap()
            .make_error(parser_utils.file.clone(), "Expected condition, found void".to_string())));
    }

    let body = parse_code(parser_utils);
    let mut else_ifs = Vec::new();
    let mut else_body = None;

    while parser_utils.tokens.get(parser_utils.index).unwrap().token_type == TokenTypes::Else {
        if parser_utils.tokens.get(parser_utils.index+1).unwrap().token_type == TokenTypes::If {
            parser_utils.index += 2;
            let effect = parse_line(parser_utils, true, false);
            if effect.is_none() {
                return Box::pin(create_error(parser_utils.tokens.get(parser_utils.index).unwrap()
                    .make_error(parser_utils.file.clone(), "Expected condition, found void".to_string())));
            }
            else_ifs.push((effect.unwrap().1, parse_code(parser_utils)));
        } else {
            parser_utils.index += 1;
            else_body = Some(parse_code(parser_utils));
        }
    }
    let adding = 1;
    parser_utils.imports.last_id += adding;
    return Box::pin(create_if(effect.unwrap().1, body, else_ifs, else_body, parser_utils.imports.last_id-adding));
}

pub fn parse_for(parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
    let name = parser_utils.tokens.get(parser_utils.index).unwrap();
    parser_utils.index += 1;
    if name.token_type != TokenTypes::Variable {
        return Box::pin(create_error(name.make_error(parser_utils.file.clone(),
                                                     "Expected variable name!".to_string())));
    }
    if parser_utils.tokens.get(parser_utils.index).unwrap().token_type != TokenTypes::In {
        return Box::pin(create_error(name.make_error(parser_utils.file.clone(),
                                                     "Missing \"in\" in for loop.".to_string())));
    }
    parser_utils.index += 1;
    let name = name.to_string(parser_utils.buffer);
    let effect = parse_line(parser_utils, true, false);
    if effect.is_none() {
        return Box::pin(create_error(parser_utils.tokens.get(parser_utils.index).unwrap().make_error(
            parser_utils.file.clone(), "Expected iterator, found void".to_string())));
    }
    let body = parse_code(parser_utils);
    parser_utils.imports.last_id += 2;
    return Box::pin(create_for(name, parser_utils.file.clone(), effect.unwrap().1,
                               body, parser_utils.imports.last_id - 1,
    parser_utils.syntax.clone(), Box::new(parser_utils.imports.clone())));
}

#[async_recursion]
async fn create_if(effect: ParsingFuture<Effects>, body: ParsingFuture<CodeBody>,
                   mut else_ifs: Vec<(ParsingFuture<Effects>, ParsingFuture<CodeBody>)>,
                   else_body: Option<ParsingFuture<CodeBody>>, mut id: u32) -> Result<Effects, ParsingError> {
    let body = body.await?;
    let end = CodeBody::new(Vec::new(), id.to_string());

    let mut else_body = if let Some(body) = else_body {
        Some(body.await?)
    } else if !else_ifs.is_empty() {
        Some(CodeBody::new(Vec::new(), id.to_string()))
    } else {
        None
    };

    let if_jumping = if let Some(body) = &else_body {
        body.label.clone()
    } else {
        end.label.clone()
    };

    let mut top = CodeBody::new(
        vec!(Expression::new(ExpressionType::Line, Effects::CompareJump(
        Box::new(effect.await?), body.label.clone(), if_jumping
    )), Expression::new(ExpressionType::Line, Effects::CodeBody(body))),
                                              id.to_string());
    id += 1;
    while !else_ifs.is_empty() {
        let (effect, body) = else_ifs.remove(0);
        let mut body = body.await?;
        body.expressions.push(Expression::new(ExpressionType::Line,
                                                     Effects::Jump(top.label.clone())));
        else_body.as_mut().unwrap().expressions.push(Expression::new(ExpressionType::Line,
        Effects::Jump(top.label.clone())));
        let inner = CodeBody::new(
            vec!(Expression::new(ExpressionType::Line,
                                 Effects::CompareJump(Box::new(effect.await?),
                                                      body.label.clone(),
                                                      else_body.as_ref().unwrap().label.clone())),
            Expression::new(ExpressionType::Line, Effects::CodeBody(body)),
            Expression::new(ExpressionType::Line, Effects::CodeBody(else_body.unwrap()))),
            id.to_string());
        else_body = Some(inner);
        id += 1;
    }

    if let Some(body) = else_body {
        top.expressions.push(Expression::new(ExpressionType::Line,
                                             Effects::CodeBody(body)));
    }

    top.expressions.push(
        Expression::new(ExpressionType::Line, Effects::CodeBody(end)));
    return Ok(Effects::CodeBody(top));
}

async fn create_for(name: String, file: String, effect: ParsingFuture<Effects>,
                    body: impl Future<Output=Result<CodeBody, ParsingError>>, id: u32,
                    syntax: Arc<Mutex<Syntax>>, name_resolver: Box<dyn NameResolver>) -> Result<Effects, ParsingError> {
    let mut top = Vec::new();
    let mut body = body.await?;
    let effect = effect.await?;
    body.expressions.insert(0, Expression::new(ExpressionType::Line,
    Effects::Set(Box::new(Effects::LoadVariable(name)), Box::new(Effects::MethodCall(
        Syntax::get_function(syntax.clone(),
                            ParsingError::new(file.clone(), (0, 0), 0, (0, 0), 0,
                                              "No core found! Report this!".to_string()),
                            "iter::Iter::next".to_string(),
                            false, name_resolver.boxed_clone()).await?,
        vec!(effect.clone()))))));

    top.push(Expression::new(ExpressionType::Line, Effects::CompareJump(Box::new(Effects::MethodCall(
        Syntax::get_function(syntax.clone(),
                            ParsingError::new(file, (0, 0), 0, (0, 0), 0,
                                              "No core found! Report this!".to_string()),
                            "iter::Iter::has_next".to_string(),
                            false, name_resolver.boxed_clone()).await?,
        vec!(effect))),
                                  body.label.clone(), (id + 1).to_string())));
    top.push(Expression::new(ExpressionType::Line, Effects::CodeBody(body)));

    return Ok(Effects::CodeBody(CodeBody::new(top, id.to_string())));
}

async fn create_error(error: ParsingError) -> Result<Effects, ParsingError> {
    return Err(error);
}
