use std::future::Future;
use std::process::Output;
use std::sync::{Arc, Mutex};
use syntax::blocks::{ForStatement, IfStatement, SwitchStatement};
use syntax::code::{Effect, Effects, Expression, ExpressionType};
use syntax::function::CodeBody;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use syntax::type_resolver::TypeResolver;
use crate::code::parse_effect;
use crate::imports::ImportManager;
use crate::parser::ParseInfo;
use crate::util::parse_code_block;

pub fn parse_if(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                -> Result<impl Future<Output=Effects>, ParsingError> {
    let type_manager = type_manager.clone();
    let effect = parse_effect(syntax, import_manager, parsing, &[b'{', b'}', b';'])?;
    parsing.index -= 1;

    let if_body = parse_code_block(syntax, import_manager, parsing)?;
    let mut else_ifs = Vec::new();
    let mut else_body = None;

    while parsing.matching("elseif") {
        let effect = parse_effect(type_manager, import_manager, parsing, &[b'{', b'}', b';'])?;
        parsing.index -= 1;
        else_ifs.push((effect, parse_code_block(type_manager, import_manager, parsing)?));
    }

    if parsing.matching("else") {
        else_body = Some(parse_code_block(type_manager, import_manager, parsing)?);
    }

    import_manager.code_block_id += 1;
    return Ok(async_if(import_manager.code_block_id, effect, if_body, else_ifs, else_body));
}

async fn async_if(label: u32, effect: impl Future<Output=Effects>, if_body: impl Future<Output=CodeBody>,
                  else_ifs: Vec<(impl Future<Output=Effects>, impl Future<Output=CodeBody>)>,
                  else_body: Option<impl Future<Output=Effects>>) -> Effects {
    let end = CodeBody::new(Vec::new(), label.to_string());
    let mut body = if_body.await;
    todo!();
    return Effects::CodeBody(body);
}

pub fn parse_for(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                 -> Result<impl Future<Output=Effects>, ParsingError> {
    parsing.next_included();
    parsing.index -= 1;
    let var_name = match parsing.parse_to_space() {
        Some(found) => found,
        None => {
            parsing.create_error("Expected variable name in for".to_string());
            return None;
        }
    };

    if !parsing.matching("in") {
        parsing.create_error("For loop needs \"in\"".to_string());
        return None;
    }

    let iterating = parse_effect(type_manager, parsing, &[b'{', b'}', b';']);

    if parsing.buffer[parsing.index - 1] != b'{' {
        parsing.create_error("Unexpected end to for loop statement!".to_string());
        return None;
    }
    let iterating = match iterating {
        Some(iterating) => iterating,
        None => {
            parsing.create_error("Couldn't find effect!".to_string());
            return None;
        }
    };

    parsing.index -= 1;
    let code = match parse_code_block(type_manager, parsing) {
        Some(code) => code,
        None => {
            parsing.create_error("Expected code body".to_string());
            return None;
        }
    };
    return Some(Effects::ForStatement(Box::new(ForStatement::new(var_name, iterating, code))));
}

pub fn parse_switch(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                    -> Result<impl Future<Output=Effects>, ParsingError> {
    let effect = match parse_effect(type_manager, parsing, &[b'{', b'}', b';']) {
        Some(effect) => effect,
        None => {
            parsing.create_error("Expected effect!".to_string());
            return None;
        }
    };

    if parsing.buffer[parsing.index - 1] != b'{' {
        parsing.create_error("Unexpected end to switch!".to_string());
        return None;
    }

    let mut conditions = Vec::new();
    while !parsing.matching("}") {
        let condition = match parse_effect(type_manager, parsing, &[b'{', b'}', b';']) {
            Some(effect) => effect,
            None => {
                parsing.create_error("Expected effect!".to_string());
                return None;
            }
        };
        if !parsing.matching("=>") {
            parsing.create_error("Expected => before switch case body".to_string());
            return None;
        }
        let body;
        if parsing.matching("{") {
            parsing.index -= 1;
            body = match parse_code_block(type_manager, parsing) {
                Some(body) => Effects::CodeBody(Box::new(body)),
                None => {
                    parsing.create_error("Expected code body!".to_string());
                    return None;
                }
            };
        } else {
            body = match parse_effect(type_manager, parsing, &[b',', b'}']) {
                Some(body) => body,
                None => {
                    parsing.create_error("Expected effect!".to_string());
                    return None;
                }
            };
        }
        conditions.push((condition, body));
    }

    return Some(Effects::SwitchStatement(Box::new(SwitchStatement::new(effect, conditions, parsing.loc()))));
}