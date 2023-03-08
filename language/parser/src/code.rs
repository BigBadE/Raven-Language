use std::mem;
use ast::code::{AssignVariable, Effects, Expression, ExpressionType, MethodCall, OperatorEffect, VariableLoad};
use ast::type_resolver::TypeResolver;
use crate::conditional::parse_if;
use crate::literal::{parse_ident, parse_number, parse_with_references};
use crate::parser::ParseInfo;
use crate::util::{find_if_first, parse_arguments, parse_code_block};

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
        let given_type = find_if_first(parsing, b':', b'=');

        match parsing.parse_to(b'=') {
            Some(name) => {
                assigning = Some((name, given_type))
            }
            None => {
                parsing.create_error("Missing name for variable assignment".to_string());
                return None;
            }
        }
    }

    if parsing.matching("if") {
        last = parse_if(type_manager, parsing);
    } else {
        while let Some(next) = parsing.next_included() {
            match next {
                _ if escape.contains(&next) => break,
                b'{' => {
                    parsing.index -= 1;
                    match parse_code_block(type_manager, parsing) {
                        Some(body) => last = Some(Effects::CodeBody(Box::new(body))),
                        None => {
                            parsing.create_error("Invalid code block!".to_string());
                            return None;
                        }
                    }
                }
                b'(' => {
                    last = Some(Effects::Wrapped(Box::new(
                        parse_effect(type_manager, parsing, &[b')', b'}', b';'])?)));
                    if parsing.buffer[parsing.index - 1] == b';' || parsing.buffer[parsing.index - 1] == b'}' {
                        parsing.create_error("Missing end of parenthesis!".to_string());
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
                            let location = parsing.loc();
                            last = Some(Effects::MethodCall(Box::new(
                                MethodCall::new(last, found,
                                                parse_arguments(type_manager, parsing), location))));
                        }
                        _ => {
                            parsing.create_error("Unexpected character".to_string());
                        }
                    }
                }
                val if (val > b'a' && val < b'z') || (val > b'A' && val < b'Z') => {
                    parsing.index -= 1;
                    let name = parse_with_references(parsing);
                    match parsing.buffer[parsing.index] {
                        b'!' => todo!(),
                        _ => {
                            parsing.index -= 1;
                            last = Some(Effects::VariableLoad(Box::new(VariableLoad::new(name, parsing.loc()))));
                        }
                    }
                }
                _ => {
                    parsing.index -= 1;
                    match parse_operator(type_manager, parsing, &mut last, escape) {
                        Some(mut operator) => last = Some(match last {
                            Some(_found) => assign_with_priority(operator),
                            None => Effects::OperatorEffect(operator)
                        }),
                        None => continue
                    }
                    break;
                }
            }
        }
    }

    return match assigning {
        Some((name, given_type)) => match last {
            Some(last) => Some(Effects::AssignVariable(Box::new(
                AssignVariable::new(name, given_type, last, parsing.loc())))),
            None => last
        },
        None => last
    };
}

fn parse_operator(type_manager: &dyn TypeResolver, parsing: &mut ParseInfo,
                  last: &mut Option<Effects>, escape: &[u8]) -> Option<Box<OperatorEffect>> {
    for (operation, name) in type_manager.get_operations() {
        let location = parsing.loc();
        let mut op_parsing = ParseInfo::new(operation.as_bytes());
        let mut effects = Vec::new();
        let add_last;
        //Add last if needed
        if op_parsing.matching("{}") {
            match last {
                Some(last) => {
                    add_last = true;
                },
                None => continue
            }
        } else {
            if last.is_some() {
                continue
            }
            add_last = false;
        }

        loop {
            if op_parsing.matching("{}") {
                if op_parsing.index == op_parsing.len {
                    effects.push(match parse_effect(type_manager, parsing, escape) {
                        Some(effect) => effect,
                        None => continue
                    });
                } else {
                    let effect = match parse_effect(type_manager, parsing,
                                              &[op_parsing.buffer[op_parsing.len+1], b';', b'}']) {
                        Some(effect) => effect,
                        None => continue
                    };

                    if op_parsing.buffer[op_parsing.len] == ';' || op_parsing.buffer[op_parsing.len] == b'}' {

                    }
                }
            } else {
                match op_parsing.next_included() {
                    Some(comparing) => match parsing.next_included() {
                        Some(comparing_against) => if comparing_against != comparing {
                            continue
                        },
                        None => continue
                    }
                    None => {
                        if add_last {
                            effects.insert(0, last.unwrap());
                        }
                        return Some(Box::new(OperatorEffect::new(
                            type_manager.get_function(name).unwrap(), effects, location)))
                    }
                }
            }
        }
    }
    return None;
}

fn assign_with_priority(mut operator: Box<OperatorEffect>) -> Effects {
    //This will be overwritten
    let mut temp_rhs = Effects::NOP();
    mem::swap(&mut temp_rhs, operator.rhs.as_mut().unwrap());

    //If the right side has more priority, it must be swapped
    return if let Effects::OperatorEffect(rhs) = temp_rhs {
        if rhs.priority < operator.priority || (rhs.priority == operator.priority && rhs.parse_left) {
            //1 / (2 + 3)
            let mut temp = Effects::NOP();
            //1 / {}, temp = 2 + 3
            mem::swap(&mut Effects::OperatorEffect(rhs), &mut temp);
            if let Effects::OperatorEffect(mut value) = temp {
                //1 / 2, temp = {} + 3
                mem::swap(&mut value.lhs, &mut operator.rhs);

                let mut effect = Effects::OperatorEffect(operator);
                //(1 / 2) + 3
                mem::swap(value.lhs.as_mut().unwrap(), &mut effect);

                return Effects::OperatorEffect(value);
            }
            panic!("Temp magically changed types!");
        }
        //Swap it back if this failed
        mem::swap(&mut Effects::OperatorEffect(rhs), operator.rhs.as_mut().unwrap());
        Effects::OperatorEffect(operator)
    } else {
        //Swap it back if this failed
        mem::swap(&mut temp_rhs, operator.rhs.as_mut().unwrap());
        Effects::OperatorEffect(operator)
    }
}