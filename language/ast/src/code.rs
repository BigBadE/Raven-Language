use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::function::Arguments;

pub struct Expression {
    pub effect: Effects
}

pub struct Field {
    pub name: Ident,
    pub field_type: Ident
}

impl Expression {
    pub fn new(effect: Effects) -> Self {
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
        return write!(f, "{};\n", self.effect.unwrap());
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect: Display {
    fn is_return(&self) -> bool;
}

pub enum Effects {
    ReturnEffect(Box<ReturnEffect>),
    MethodCall(Box<MethodCall>),
    VariableLoad(Box<VariableLoad>),
    MathEffect(Box<MathEffect>),
    FloatEffect(Box<NumberEffect<f64>>),
    IntegerEffect(Box<NumberEffect<u64>>),
}

impl Effects {
    pub fn unwrap(&self) -> &dyn Effect {
        return match self {
            Effects::ReturnEffect(effect) => effect.as_ref(),
            Effects::MethodCall(effect) => effect.as_ref(),
            Effects::VariableLoad(effect) => effect.as_ref(),
            Effects::MathEffect(effect) => effect.as_ref(),
            Effects::FloatEffect(effect) => effect.as_ref(),
            Effects::IntegerEffect(effect) => effect.as_ref()
        };
    }
}

impl Display for Effects {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return Display::fmt(self.unwrap(), f);
    }
}

pub struct ReturnEffect {
    pub effect: Effects
}

impl ReturnEffect {
    pub fn new(effect: Effects) -> Self {
        return Self {
            effect
        }
    }
}

impl Effect for ReturnEffect {
    fn is_return(&self) -> bool {
        return true;
    }
}

impl Display for ReturnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "return {}", self.effect)
    }
}

pub struct MethodCall {
    pub calling: Effects,
    pub method: Ident,
    pub arguments: Arguments
}

impl MethodCall {
    pub fn new(calling: Effects, method: Ident, arguments: Arguments) -> Self {
        return Self {
            calling,
            method,
            arguments
        };
    }
}

impl Effect for MethodCall {
    fn is_return(&self) -> bool {
        return false;
    }
}

impl Display for MethodCall {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}.{}{}", self.calling, self.method, self.arguments);
    }
}

pub struct VariableLoad {
    pub name: Ident
}

impl VariableLoad {
    pub fn new(name: Ident) -> Self {
        return Self {
            name
        }
    }
}

impl Effect for VariableLoad {
    fn is_return(&self) -> bool {
        return false;
    }
}

impl Display for VariableLoad {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
    }
}

pub struct MathEffect {
    pub target: Effects,
    pub operator: MathOperator,
    pub effect: Effects
}

#[derive(Clone)]
pub enum MathOperator {
    PLUS = '+' as isize,
    MINUS = '-' as isize,
    DIVIDE = '/' as isize,
    MULTIPLY = '*' as isize
}

impl MathEffect {
    pub fn new(target: Effects, operator: MathOperator, effect: Effects) -> Self {
        return Self {
            target,
            operator,
            effect
        }
    }
}

impl Effect for MathEffect {
    fn is_return(&self) -> bool {
        return false;
    }
}

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

impl<T> Effect for NumberEffect<T> where T : Display {
    fn is_return(&self) -> bool {
        return false;
    }
}

impl<T> Display for NumberEffect<T> where T : Display {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.number);
    }
}