use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::function::Arguments;

pub struct Expression {
    effect: Box<dyn Effect>
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
        return write!(f, "{};", self.effect);
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect: Display {

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