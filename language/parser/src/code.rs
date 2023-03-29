use std::mem;
use ast::code::{AssignVariable, CreateStruct, Effects, Expression, ExpressionType, FieldLoad, FieldSet, MethodCall, OperatorEffect, VariableLoad};
use ast::type_resolver::TypeResolver;
use ast::types::ResolvableTypes;
use crate::conditional::{parse_for, parse_if, parse_switch};
use crate::literal::{parse_ident, parse_number, parse_with_references};
use crate::parser::ParseInfo;
use crate::util::{parse_arguments, parse_code_block, parse_struct_args};

pub fn parse_expression(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo) -> Option<Expression> {
    return if parsing.matching("return") {
        Some(Expression::new(ExpressionType::Return,
                             parse_effect(type_manager, parsing, &[b';', b'}'])
                                 .unwrap_or(Effects::NOP())))
    } else if parsing.matching("break") {
        Some(Expression::new(ExpressionType::Break,
                             parse_effect(type_manager, parsing, &[b';', b'}'])
                                 .unwrap_or(Effects::NOP())))
    } else {
        Some(Expression::new(ExpressionType::Line,
                             parse_effect(type_manager, parsing, &[b';', b'}'])?))
    };
}

pub fn parse_effect(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo, escape: &[u8]) -> Option<Effects> {
    let mut last = None;
    let mut assigning = None;
    if parsing.matching("let") {
        match parsing.parse_to(b'=') {
            Some(name) => {
                assigning = Some(name)
            }
            None => {
                parsing.create_error("Missing name for variable assignment".to_string());
                return None;
            }
        }
    }

    if parsing.matching("if") {
        last = parse_if(type_manager, parsing);
    } else if parsing.matching("for") {
        return parse_for(type_manager, parsing);
    } else if parsing.matching("switch") {
        last = parse_switch(type_manager, parsing);
    } else {
        while let Some(next) = parsing.next_included() {
            match next {
                _ if escape.contains(&next) => break,
                b'{' => {
                    if last.is_some() {
                        match last.unwrap() {
                            Effects::VariableLoad(variable_load) => {
                                let structure = variable_load.name;
                                last = Some(Effects::CreateStruct(
                                    Box::new(CreateStruct::new(ResolvableTypes::Resolving(structure),
                                                               parse_struct_args(type_manager, parsing), parsing.loc()))));
                            }
                            _ => {
                                last = None;
                                parsing.create_error("Unexpected curly bracket!".to_string());
                            }
                        }
                    } else {
                        parsing.index -= 1;
                        match parse_code_block(type_manager, parsing) {
                            Some(body) => last = Some(Effects::CodeBody(Box::new(body))),
                            None => {
                                parsing.create_error("Invalid code block!".to_string());
                                return None;
                            }
                        }
                    }
                }
                b'(' => {
                    match last {
                        Some(found) => {
                            match found {
                                Effects::VariableLoad(variable) => {
                                    last = Some(Effects::MethodCall(Box::new(
                                        MethodCall::new(None, variable.name,
                                                        parse_arguments(type_manager, parsing), parsing.loc(),
                                        ))));
                                }
                                _ => {
                                    last = None;
                                    parsing.create_error("Unknown parenthesis!".to_string());
                                }
                            }
                        }
                        None => {
                            last = Some(Effects::Wrapped(Box::new(
                                parse_effect(type_manager, parsing, &[b')', b'}', b';'])?)));
                            if parsing.buffer[parsing.index - 1] == b';' || parsing.buffer[parsing.index - 1] == b'}' {
                                parsing.create_error("Missing end of parenthesis!".to_string());
                            }
                        }
                    }
                }
                b'=' => {
                    let mut temp = parsing.clone();
                    temp.index -= 1;
                    let test = parse_effect(type_manager, &mut temp, escape);
                    if test.is_some() {
                        if let Effects::OperatorEffect(found) = test.unwrap() {
                            if found.operator.starts_with("{}=") {
                                last = Some(Effects::OperatorEffect(found));
                                break;
                            }
                        }
                    }
                    let next = parse_effect(type_manager, parsing, escape)?;
                    match last? {
                        Effects::VariableLoad(variable) =>
                            last = Some(Effects::AssignVariable(Box::new(AssignVariable::new(variable.name, next, parsing.loc())))),
                        Effects::FieldLoad(field) =>
                            last = Some(Effects::FieldSet(Box::new(FieldSet::new(field.calling, field.name, next, parsing.loc())))),
                        _ => {
                            parsing.create_error("Tried to set an unsettable value!".to_string());
                            last = None;
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
                            let location = parsing.loc();
                            last = Some(Effects::MethodCall(Box::new(
                                MethodCall::new(last, found,
                                                parse_arguments(type_manager, parsing), location))));
                        }
                        _ => {
                            last = Some(Effects::FieldLoad(Box::new(
                                FieldLoad::new(last.unwrap(), found, parsing.loc()))));
                        }
                    }
                }
                val if (val > b'a' && val < b'z') || (val > b'A' && val < b'Z') => {
                    parsing.index -= 1;
                    let name = parse_with_references(parsing);
                    match parsing.buffer[parsing.index] {
                        b'!' => todo!(),
                        _ => {
                            last = Some(Effects::VariableLoad(Box::new(VariableLoad::new(name, parsing.loc()))));
                        }
                    }
                }
                _ => {
                    parsing.index -= 1;
                    match parse_operator(type_manager, parsing, &mut last, escape) {
                        Some(operator) => last = Some(Effects::OperatorEffect(operator)),
                        None => return None
                    }
                    break;
                }
            }
        }
    }

    return match assigning {
        Some(name) => match last {
            Some(last) => Some(Effects::AssignVariable(Box::new(
                AssignVariable::new(name, last, parsing.loc())))),
            None => last
        },
        None => last
    };
}

fn parse_operator(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo,
                  last: &mut Option<Effects>, escape: &[u8]) -> Option<Box<OperatorEffect>> {
    let location = parsing.loc();
    let mut temp = parsing.clone();
    let mut output = String::new();
    let mut effects = Vec::new();

    //Skip if last is needed
    if last.is_some() {
        output += "{}";
    }

    output.push(parsing.buffer[parsing.index] as char);

    loop {
        match temp.next_included() {
            Some(comparing) => {
                match parse_effect(type_manager, &mut temp, escape) {
                    Some(effect) => {
                        if let Effects::OperatorEffect(effect) = effect {
                            output += effect.operator.as_str();
                            for found in Box::into_inner(effect).effects {
                                effects.push(found);
                            }
                            break;
                        } else {
                            effects.push(effect);
                            output += "{}";
                        }
                        break;
                    }
                    None => {
                        output.push(comparing as char);
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

    //Update parsing and return
    *parsing = temp;
    return Some(Box::new(OperatorEffect::new(output, effects, location)));
}