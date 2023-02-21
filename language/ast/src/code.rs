use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::function::Arguments;

pub struct Expression {
    pub effect: Box<dyn Effect>
}

pub struct Field {
    name: Ident,
    field_type: Ident
}

impl Expression {
    pub fn new(effect: Box<dyn Effect>) -> Self {
        return Self {
            effect
        }
    }
}

impl Field {
    pub fn new(name: Ident, field_type: Ident) -> Self {
        return Self {
            name,
            field_type
        }
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{};\n", self.effect);
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect: Display {

}

pub struct ReturnEffect {
    pub effect: Box<dyn Effect>
}

impl ReturnEffect {
    pub fn new(effect: Box<dyn Effect>) -> Self {
        return Self {
            effect
        }
    }
}

impl Effect for ReturnEffect {}

impl Display for ReturnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "return {}", self.effect)
    }
}

pub struct MethodCall {
    pub calling: Box<dyn Effect>,
    pub method: Ident,
    pub arguments: Arguments
}

impl MethodCall {
    pub fn new(calling: Box<dyn Effect>, method: Ident, arguments: Arguments) -> Self {
        return Self {
            calling,
            method,
            arguments
        };
    }
}

impl Effect for MethodCall {}

impl Display for MethodCall {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}.{}{}", self.calling, self.method, self.arguments);
    }
}

pub struct VariableLoad {
    name: Ident
}

impl VariableLoad {
    pub fn new(name: Ident) -> Self {
        return Self {
            name
        }
    }
}

impl Effect for VariableLoad {}

impl Display for VariableLoad {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
    }
}

pub struct MathEffect {
    target: Box<dyn Effect>,
    operator: MathOperator,
    effect: Box<dyn Effect>
}

#[derive(Clone)]
pub enum MathOperator {
    PLUS = '+' as isize,
    MINUS = '-' as isize,
    DIVIDE = '/' as isize,
    MULTIPLY = '*' as isize
}

impl MathEffect {
    pub fn new(target: Box<dyn Effect>, operator: MathOperator, effect: Box<dyn Effect>) -> Self {
        return Self {
            target,
            operator,
            effect
        }
    }
}

impl Effect for MathEffect {}

impl Display for MathEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{} {} {}", self.target, self.operator.clone() as u8 as char, self.effect);
    }
}

pub struct NumberEffect<T> where T : Display {
    pub number: T
}

impl<T> NumberEffect<T> where T : Display {
    pub fn new(number: T) -> Self {
        return Self {
            number
        }
    }
}

impl<T> Effect for NumberEffect<T> where T : Display {}

impl<T> Display for NumberEffect<T> where T : Display {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.number);
    }
}