use std::future::Future;
use std::sync::{Arc, Mutex};
use syntax::code::Effects;
use syntax::function::CodeBody;
use syntax::ParsingError;
use syntax::syntax::Syntax;
use crate::code::parse_effect;
use crate::imports::ImportManager;
use crate::parser::ParseInfo;
use crate::util::parse_code_block;

pub fn parse_if(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                -> Result<impl Future<Output=Result<Effects, ParsingError>>, ParsingError> {
    let effect = parse_effect(syntax, import_manager, parsing,
                              &[b'{', b'}', b';'], true)?;
    parsing.index -= 1;

    let if_body = parse_code_block(syntax, import_manager, parsing)?;
    let mut else_ifs = Vec::new();
    let mut else_body = None;

    while parsing.matching("elseif") {
        let effect = parse_effect(syntax, import_manager, parsing,
                                  &[b'{', b'}', b';'], true)?;
        parsing.index -= 1;
        else_ifs.push((effect, parse_code_block(syntax, import_manager, parsing)?));
    }

    if parsing.matching("else") {
        else_body = Some(parse_code_block(syntax, import_manager, parsing)?);
    }

    import_manager.code_block_id += 1;
    return Ok(async_if(import_manager.code_block_id, effect, if_body, else_ifs, else_body));
}

async fn async_if(label: u32, effect: impl Future<Output=Result<Effects, ParsingError>>, if_body: impl Future<Output=CodeBody>,
                  else_ifs: Vec<(impl Future<Output=Result<Effects, ParsingError>>, impl Future<Output=CodeBody>)>,
                  else_body: Option<impl Future<Output=CodeBody>>) -> Result<Effects, ParsingError> {
    let end = CodeBody::new(Vec::new(), label.to_string());
    let mut body = if_body.await;
    todo!();
    return Ok(Effects::CodeBody(body));
}

pub fn parse_for(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                 -> Result<impl Future<Output=Result<Effects, ParsingError>>, ParsingError> {
    parsing.next_included();
    parsing.index -= 1;
    let var_name = match parsing.parse_to_space() {
        Some(found) => found,
        None => {
            return Err(ParsingError::new((0, 0), (0, 0), "Expected variable name in for".to_string()));
        }
    };

    if !parsing.matching("in") {
        return Err(ParsingError::new((0, 0), (0, 0), "For loop needs \"in\"".to_string()));
    }

    let iterating = parse_effect(syntax, import_manager, parsing,
                                 &[b'{', b'}', b';'], true);

    if parsing.buffer[parsing.index - 1] != b'{' {
        return Err(ParsingError::new((0, 0), (0, 0), "Unexpected end to for loop statement!".to_string()));
    }

    parsing.index -= 1;
    let code = parse_code_block(syntax, import_manager, parsing)?;

    //TODO desugar for loop
    todo!();
}

pub fn parse_switch(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                    -> Result<impl Future<Output=Result<Effects, ParsingError>>, ParsingError> {
    let effect = parse_effect(syntax, import_manager, parsing,
                              &[b'{', b'}', b';'], true)?;

    if parsing.buffer[parsing.index - 1] != b'{' {
        return Err(ParsingError::new((0, 0), (0, 0), "Unexpected end to switch".to_string()));
    }

    let mut conditions = Vec::new();
    while !parsing.matching("}") {
        let condition = parse_effect(syntax, import_manager, parsing,
                                     &[b'{', b'}', b';'], true)?;
        if !parsing.matching("=>") {
            return Err(ParsingError::new((0, 0), (0, 0), "Expected => before switch case body".to_string()));
        }
        let body;
        if parsing.matching("{") {
            parsing.index -= 1;
            body = to_effect(parse_code_block(syntax, import_manager, parsing)?);
        } else {
            body = parse_effect(syntax, import_manager, parsing, &[b',', b'}'], true)?;
        }
        conditions.push((condition, body));
    }

    //TODO add switch desugaring
    todo!();
}

async fn to_effect(block: impl Future<Output=CodeBody>) -> Result<Effects, ParsingError> {
    return Ok(Effects::CodeBody(block.await));
}