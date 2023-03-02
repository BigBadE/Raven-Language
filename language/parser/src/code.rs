use std::mem;
use ast::code::{AssignVariable, Effects, Expression, ExpressionType, MethodCall, OperatorEffect, VariableLoad};
use ast::program::Program;
use crate::literal::{parse_ident, parse_number, parse_with_references};
use crate::parser::ParseInfo;
use crate::types::ParsingTypeResolver;
use crate::util::{find_if_first, parse_arguments};

pub fn parse_expression(program: &Program, type_manager: &ParsingTypeResolver, parsing: &mut ParseInfo) -> Option<Expression> {
    return if parsing.matching("return") {
        Some(Expression::new(ExpressionType::Return,
                             parse_effect(program, type_manager, parsing, &[b';', b'}'])
                                 .unwrap_or(Effects::NOP())))
    } else if parsing.matching("break") {
        Some(Expression::new(ExpressionType::Break,
                             parse_effect(program, type_manager, parsing, &[b';', b'}'])
                                 .unwrap_or(Effects::NOP())))
    } else {
        Some(Expression::new(ExpressionType::Line,
                             parse_effect(program, type_manager, parsing, &[b';', b'}'])?))
    };
}

pub fn parse_effect(program: &Program, type_manager: &ParsingTypeResolver, parsing: &mut ParseInfo, escape: &[u8]) -> Option<Effects> {
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

    while let Some(next) = parsing.next_included() {
        match next {
            _ if escape.contains(&next) => break,
            b'(' => last = parse_effect(program, type_manager, parsing, &[b')']),
            b'0'..=b'9' => {
                parsing.index -= 1;
                last = parse_number(parsing)
            }
            b'.' => {
                let found = parse_ident(parsing);
                match parsing.buffer[parsing.index] {
                    b'(' => {
                        let location = parsing.loc();
                        last = Some(Effects::MethodCall(Box::new(MethodCall::new(last, found,
                            parse_arguments(program, type_manager, parsing), location))));
                    }
                    _ => {
                        parsing.create_error("Unexpected character".to_string());
                    }
                }
            },
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
                match parse_operator(program, type_manager, parsing, &mut last, escape) {
                    Some(operator) => last = Some(match last {
                        Some(found) => match found {
                            Effects::OperatorEffect(last_found) =>
                                Effects::OperatorEffect(assign_with_priority(last_found, operator)),
                            _ => Effects::OperatorEffect(operator)
                        },
                        None => Effects::OperatorEffect(operator)
                    }),
                    None => continue
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

fn parse_operator(program: &Program, type_manager: &ParsingTypeResolver, parsing: &mut ParseInfo,
                  last: &mut Option<Effects>, escape: &[u8]) -> Option<Box<OperatorEffect>> {
    for (operation, name) in &program.operations {
        let location = parsing.loc();
        if parsing.matching(operation.as_str()) {
            let function = &program.static_functions;
            let found = function.get(name).unwrap();

            match found.attributes.get("right_sided") {
                Some(_attribute) => {
                    match last.as_mut() {
                        Some(last) => {
                            let mut temp = Effects::NOP();
                            mem::swap(last, &mut temp);
                            return Some(Box::new(
                                OperatorEffect::new(&operation, found, Some(temp), None, parsing.loc())));
                        },
                        None => {
                            continue
                        }
                    }
                },
                None => {}
            }

            let effect = match parse_effect(program, type_manager, parsing, escape) {
                Some(effect) => effect,
                None => {
                    parsing.create_error("Unexpected end of line!".to_string());
                    return None;
                }
            };

            return match last.as_mut() {
                Some(last) => {
                    let mut temp = Effects::NOP();
                    mem::swap(last, &mut temp);
                    if !found.check_args(type_manager, &vec!(&temp, &effect)) {
                        continue
                    }

                    Some(Box::new(OperatorEffect::new(operation, found,
                                                 Some(temp), Some(effect), location)))
                },
                None => {
                    if !found.check_args(type_manager, &vec!(&effect)) {
                        continue
                    }
                    Some(Box::new(OperatorEffect::new(operation, found, None,
                                                      Some(effect), location)))
                }
            }
        }
    }

    return None;
}

fn assign_with_priority(mut lhs: Box<OperatorEffect>, mut rhs: Box<OperatorEffect>) -> Box<OperatorEffect> {
    //If the left side is higher priority or equal and parse left, add the lhs to the left of the rhs
    return if lhs.priority > rhs.priority ||
        (lhs.priority == rhs.priority && rhs.parse_left) {
        mem::swap(rhs.rhs.as_mut().unwrap(), &mut Effects::OperatorEffect(lhs));
        rhs
    } else {
        //If not, swap the right value of the lhs into the left value of the rhs
        //and put the rhs as the right value of the lhs
        // 1 + 2, {} / 3
        let mut swapping = Effects::NOP();
        // 1 + {}, 2, {} / 3
        mem::swap(lhs.rhs.as_mut().unwrap(), &mut swapping);
        // 1 + {}, 2 / 3
        mem::swap(rhs.lhs.as_mut().unwrap(), &mut swapping);
        // 1 + (2 / 3)
        mem::swap(lhs.rhs.as_mut().unwrap(), &mut Effects::OperatorEffect(rhs));

        lhs
    };
}