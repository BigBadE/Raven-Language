use std::future::Future;
use std::sync::{Arc, Mutex};
use syntax::async_util::{FunctionGetter, NameResolver};

use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::parser::code_parser::{parse_code, parse_line, ParsingFuture};

use crate::{ParserUtils, TokenTypes};

pub fn parse_for(parser_utils: &mut ParserUtils) -> ParsingFuture<Effects> {
    let name = parser_utils.tokens.remove(0);
    if name.token_type != TokenTypes::Variable {
        return Box::pin(create_error(name.make_error(parser_utils.file.clone(),
                                                     "Expected variable name!".to_string())));
    }
    if parser_utils.tokens.remove(0).token_type != TokenTypes::In {
        return Box::pin(create_error(name.make_error(parser_utils.file.clone(),
                                                     "Missing \"in\" in for loop.".to_string())));
    }
    let name = name.to_string(parser_utils.buffer);
    let effect = parse_line(parser_utils, true, false);
    if effect.is_none() {
        return Box::pin(create_error(parser_utils.tokens.get(0).unwrap().make_error(
            parser_utils.file.clone(), "Expected iterator, found void".to_string())));
    }
    let body = parse_code(parser_utils);
    parser_utils.imports.last_id += 1;
    return Box::pin(create_for(name, parser_utils.file.clone(), effect.unwrap().1,
                               body, parser_utils.imports.last_id - 1,
    parser_utils.syntax.clone(), Box::new(parser_utils.imports.clone())));
}

async fn create_for(name: String, file: String, effect: ParsingFuture<Effects>,
                    body: impl Future<Output=Result<CodeBody, ParsingError>>, id: u32,
                    syntax: Arc<Mutex<Syntax>>, name_resolver: Box<dyn NameResolver>) -> Result<Effects, ParsingError> {
    let mut top = Vec::new();
    let mut body = body.await?;
    let effect = effect.await?;
    body.expressions.insert(0, Expression::new(ExpressionType::Line,
    Effects::Set(Box::new(Effects::LoadVariable(name)), Box::new(Effects::MethodCall(
        FunctionGetter::new(syntax.clone(),
                            ParsingError::new(file.clone(), (0, 0), 0, (0, 0), 0,
                                              "No core found! Report this!".to_string()),
                            "iter::Iter::next".to_string(),
                            name_resolver.boxed_clone()).await?,
        vec!(effect.clone()))))));

    top.push(Expression::new(ExpressionType::Line, Effects::CompareJump(Box::new(Effects::MethodCall(
        FunctionGetter::new(syntax.clone(),
                            ParsingError::new(file, (0, 0), 0, (0, 0), 0,
                                              "No core found! Report this!".to_string()),
                            "iter::Iter::has_next".to_string(),
                            name_resolver.boxed_clone()).await?,
        vec!(effect))),
                                  body.label.clone(), (id + 1).to_string())));
    top.push(Expression::new(ExpressionType::Line, Effects::CodeBody(body)));

    return Ok(Effects::CodeBody(CodeBody::new(top, id.to_string())));
}

async fn create_error(error: ParsingError) -> Result<Effects, ParsingError> {
    return Err(error);
}
