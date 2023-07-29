use syntax::code::{Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use crate::parser::code_parser::{parse_code, parse_line};

use crate::{ParserUtils, TokenTypes};

pub fn parse_if(parser_utils: &mut ParserUtils) -> Result<Expression, ParsingError> {
    let effect = parse_line(parser_utils, true, false)?;
    if effect.is_none() {
        return Err(parser_utils.tokens.get(parser_utils.index).unwrap()
            .make_error(parser_utils.file.clone(), "Expected condition, found void".to_string()));
    }

    if parser_utils.tokens.get(parser_utils.index-1).unwrap().token_type != TokenTypes::BlockStart {
        return Err(parser_utils.tokens.get(parser_utils.index).unwrap()
            .make_error(parser_utils.file.clone(), "Expected body, found void".to_string()));
    }

    let (mut returning, body) = parse_code(parser_utils)?;
    let mut else_ifs = Vec::new();
    let mut else_body = None;

    while parser_utils.tokens.get(parser_utils.index).unwrap().token_type == TokenTypes::Else {
        if parser_utils.tokens.get(parser_utils.index+1).unwrap().token_type == TokenTypes::If {
            parser_utils.index += 2;
            let effect = parse_line(parser_utils, true, false)?;
            if effect.is_none() {
                return Err(parser_utils.tokens.get(parser_utils.index).unwrap()
                    .make_error(parser_utils.file.clone(), "Expected condition, found void".to_string()));
            }
            let (other_returning, body) = parse_code(parser_utils)?;
            if other_returning != returning {
                returning = ExpressionType::Line;
            }
            else_ifs.push((effect.unwrap().effect, body));
        } else if parser_utils.tokens.get(parser_utils.index+1).unwrap().token_type == TokenTypes::BlockStart {
            parser_utils.index += 2;
            let (other_returning, body) = parse_code(parser_utils)?;
            if other_returning != returning {
                returning = ExpressionType::Line;
            }
            else_body = Some(body);
            break
        } else {
            return Err(parser_utils.tokens.get(parser_utils.index).unwrap()
                .make_error(parser_utils.file.clone(), "Expected block!".to_string()))
        }
    }

    if else_body.is_none() {
        returning = ExpressionType::Line;
    }

    let adding = 1 + else_ifs.len() as u32 + else_body.is_some() as u32;
    parser_utils.imports.last_id += adding;
    return Ok(Expression::new(returning, create_if(effect.unwrap().effect, body, else_ifs, else_body,
                                          parser_utils.imports.last_id-adding)?));
}

pub fn parse_for(parser_utils: &mut ParserUtils) -> Result<Effects, ParsingError> {
    let name = parser_utils.tokens.get(parser_utils.index).unwrap();
    parser_utils.index += 1;
    if name.token_type != TokenTypes::Variable {
        return Err(name.make_error(parser_utils.file.clone(),
                                                     "Expected variable name!".to_string()));
    }
    if parser_utils.tokens.get(parser_utils.index).unwrap().token_type != TokenTypes::In {
        return Err(name.make_error(parser_utils.file.clone(),
                                                     "Missing \"in\" in for loop.".to_string()));
    }
    parser_utils.index += 1;
    let name = name.to_string(parser_utils.buffer);
    let effect = parse_line(parser_utils, true, false)?;
    if effect.is_none() {
        return Err(parser_utils.tokens.get(parser_utils.index).unwrap().make_error(
            parser_utils.file.clone(), "Expected iterator, found void".to_string()));
    }
    let body = parse_code(parser_utils)?.1;
    parser_utils.imports.last_id += 2;
    return create_for(name, effect.unwrap().effect,
                               body, parser_utils.imports.last_id - 2);
}

fn create_if(effect: Effects, body: CodeBody,
                   mut else_ifs: Vec<(Effects, CodeBody)>,
                   else_body: Option<CodeBody>, mut id: u32) -> Result<Effects, ParsingError> {
    let body = body;
    let end = CodeBody::new(Vec::new(), id.to_string() + "end");

    let mut else_body = if let Some(body) = else_body {
        Some(body)
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
        Box::new(effect), body.label.clone(), if_jumping
    )), Expression::new(ExpressionType::Line, Effects::CodeBody(body))),
                                              id.to_string());
    id += 1;
    while !else_ifs.is_empty() {
        let (effect, mut body) = else_ifs.remove(0);
        body.expressions.push(Expression::new(ExpressionType::Line,
                                                     Effects::Jump(top.label.clone())));
        else_body.as_mut().unwrap().expressions.push(Expression::new(ExpressionType::Line,
        Effects::Jump(top.label.clone())));
        let inner = CodeBody::new(
            vec!(Expression::new(ExpressionType::Line,
                                 Effects::CompareJump(Box::new(effect),
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

fn create_for(name: String, effect: Effects, mut body: CodeBody, id: u32) -> Result<Effects, ParsingError> {
    let mut top = Vec::new();
    body.expressions.insert(0, Expression::new(ExpressionType::Line,
    Effects::CreateVariable(name.clone(), Box::new(Effects::ImplementationCall(
        Box::new(effect.clone()), "iter::Iter".to_string(), "iter::next".to_string(), vec!())))));
    body.expressions.push(Expression::new(ExpressionType::Line, Effects::Jump(id.to_string())));

    top.push(Expression::new(ExpressionType::Line, Effects::CompareJump(Box::new(Effects::ImplementationCall(
        Box::new(effect), "iter::Iter".to_string(), "iter::has_next".to_string(), vec!())),
                                  body.label.clone(), id.to_string() + "end")));
    top.push(Expression::new(ExpressionType::Line, Effects::CodeBody(body)));

    return Ok(Effects::CodeBody(CodeBody::new(top, id.to_string())));
}