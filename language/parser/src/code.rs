use std::future::Future;
use std::mem;
use std::sync::{Arc, Mutex};
use anyhow::Error;
use syntax::code::{AssignVariable, CreateStruct, Effects, Expression, ExpressionType, FieldLoad, FieldSet, MethodCall, OperatorEffect, VariableLoad};
use syntax::ParsingError;
use syntax::syntax::Syntax;
use syntax::type_resolver::TypeResolver;
use syntax::types::ResolvableTypes;
use crate::async_code::{async_create_struct, async_field_load, async_method_call, async_parse_expression, async_parse_operator, async_set};
use crate::conditional::{parse_for, parse_if, parse_switch};
use crate::imports::ImportManager;
use crate::literal::{parse_ident, parse_number, parse_with_references};
use crate::parser::ParseInfo;
use crate::util::{parse_arguments, parse_code_block, parse_struct_args};

pub fn parse_expression(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo)
                        -> Result<impl Future<Output=Effects>, ParsingError> {
    let expression_type = if parsing.matching("return") {
        ExpressionType::Return
    } else if parsing.matching("break") {
        ExpressionType::Break
    } else {
        ExpressionType::Line
    };
    let handle = syntax.lock().unwrap().manager.handle().clone();
    return Ok(handle.spawn(
        async_parse_expression(expression_type, parse_effect(syntax, import_manager, parsing, &[b';', b'}'], true)?)));
}

pub fn parse_effect(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo,
                    escape: &[u8], operators: bool)
                    -> Result<impl Future<Output=Effects>, ParsingError> {
    let mut last = None;

    if parsing.matching("if") {
        last = Some(parse_if(syntax, import_manager, parsing)?);
    } else if parsing.matching("for") {
        return parse_for(syntax, import_manager, parsing);
    } else if parsing.matching("switch") {
        last = parse_switch(syntax, import_manager, parsing)?;
    } else if parsing.matching("let") {
        return match parsing.parse_to(b'=') {
            Some(name) => {
                Ok(async_set(name, parse_effect(syntax, import_manager, parsing, escape, operators)?))
            }
            None => {
                parsing.create_error("Missing name for variable assignment".to_string());
                Ok(())
            }
        };
    }
    while let Some(next) = parsing.next_included() {
        match next {
            _ if escape.contains(&next) => break,
            b'{' => {
                if let Some(found) = last {
                    match found {
                        Effects::Load(_from, name) => {
                            last = Some(async_create_struct(syntax, Box::new(import_manager.clone()), name,
                                                            parse_struct_args(syntax, import_manager, parsing)?));
                        }
                        _ => {
                            last = None;
                            parsing.create_error("Unexpected curly bracket!".to_string());
                        }
                    }
                } else {
                    parsing.index -= 1;
                    last = parse_code_block(syntax, import_manager, parsing)?;
                }
            }
            b'(' => {
                if let Some(found) = last {
                    last = Some(async_method_call(syntax, Box::new(import_manager.clone()), found, variable.name,
                                                  parse_arguments(syntax, import_manager, parsing)?));
                } else {
                    last = parse_effect(syntax, import_manager, parsing, escape, operators)?;
                    if parsing.buffer[parsing.index - 1] == b';' || parsing.buffer[parsing.index - 1] == b'}' {
                        parsing.create_error("Missing end of parenthesis!".to_string());
                    }
                }
            }
            b'0'..=b'9' => {
                parsing.index -= 1;
                last = parse_number(parsing)
            }
            b'.' => {
                let found = parse_ident(parsing);

                match parsing.buffer[parsing.index] {
                    b'(' => {
                        parsing.index += 1;
                        last = Some(async_method_call(
                            syntax, Box::new(import_manager.clone()), last, found,
                            parse_arguments(syntax, import_manager, parsing)?));
                    }
                    _ => {
                        last = Some(async_field_load(last, found));
                    }
                }
            }
            val
            if (val > b'a' && val < b'z') || (val > b'A' && val < b'Z') => {
                parsing.index -= 1;
                let name = parse_with_references(parsing);
                match parsing.buffer[parsing.index] {
                    //TODO macros
                    b'!' => todo!(),
                    _ => {
                        last = Some(Effects::Load(None, name));
                    }
                }
            }
            _ => {
                if operators {
                    parsing.index -= 1;
                    last = parse_operator(syntax, import_manager, parsing, &mut last, escape)?;
                } else {
                    //The operator ignores this error, so the user should never see this.
                    return Err(ParsingError::new((0, 0), (0, 0),
                                                 "If your seeing this, it's not your fault! Report this please!".to_string()));
                }
            }
        }
    }
    return Ok(last.unwrap());
}

fn parse_operator(syntax: &Arc<Mutex<Syntax>>, import_manager: &mut ImportManager, parsing: &mut ParseInfo,
                  last: &mut Option<impl Future<Output=Effects>>, escape: &[u8])
                  -> Result<impl Future<Output=Effects>, ParsingError> {
    let mut temp = parsing.clone();
    let mut output = String::new();
    let mut effects = Vec::new();

    if last.is_some() {
        output += "{}";
    }

    output.push(parsing.buffer[parsing.index] as char);

    loop {
        match temp.next_included() {
            Some(comparing) => {
                match parse_effect(syntax, import_manager, &mut temp, escape, false) {
                    Ok(effect) => {
                        if output.ends_with("{}") {
                            break
                        } else {
                            effects.push(effect);
                            output += "{}";
                            *parsing = temp.clone();
                        }
                    }
                    Err(_) => {
                        output.push(comparing as char);
                        *parsing = temp.clone();
                    }
                }
            }
            None => break
        }
    }

    //Since last isn't owned, swap is needed, which can only be done after every argument
    //is type checked.
    if last.is_some() {
        let mut temp_last = Effects::NOP();
        mem::swap(&mut temp_last, last.as_mut().unwrap());
        effects.push(temp_last);
    }

    return async_parse_operator(syntax, Box::new(import_manager.clone()), output, effects);
}