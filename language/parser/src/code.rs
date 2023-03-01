use ast::code::{Effects, Expression, ExpressionType};
use crate::parser::{ParseError, ParseInfo};

pub fn parse_expression(errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<Expression> {
    let mut subparser = match parsing.subparse(b';') {
        Some(parsing) => parsing,
        None => {
            errors.push(parsing.create_error("".to_string()));
            return None;
        }
    };

    return if parsing.matching("return") {
        Some(Expression::new(ExpressionType::Return, parse_effect(errors, &mut subparser).unwrap_or(Effects::NOP())))
    } else if parsing.matching("break") {
        Some(Expression::new(ExpressionType::Break, parse_effect(errors, &mut subparser).unwrap_or(Effects::NOP())))
    } else {
        Some(Expression::new(ExpressionType::Line, parse_effect(errors, &mut subparser)?))
    };
}

fn parse_effect(errors: &mut Vec<ParseError>, parsing: &mut ParseInfo) -> Option<Effects> {
    let mut last = None;


}