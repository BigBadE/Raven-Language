use std::fmt::{Display, Formatter};
use crate::class_type::ClassType;
use crate::function_type::Function;

pub mod basic_types;
pub mod class_type;
pub mod code;
pub mod function_type;

pub enum TopElement {
    Struct(ClassType),
    Function(Function)
}

impl Display for TopElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            TopElement::Struct(class_type) => Display::fmt(class_type, f),
            TopElement::Function(function) => Display::fmt(function, f)
        }
    }
}

#[derive(Clone)]
pub enum Modifier {
    Public = 0b0000_0001
}

fn get_modifier(modifiers: &[Modifier]) -> u8 {
    let mut sum = 0;
    for modifier in modifiers {
        sum += modifier.clone() as u8;
    }

    return sum;
}

fn is_modifier(modifiers: u8, target: Modifier) -> bool {
    let target = target as u8;
    return modifiers & target == target;
}