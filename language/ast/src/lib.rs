use std::fmt::{Display, Formatter};
use crate::r#struct::Struct;
use crate::function::Function;

pub mod basic_types;
pub mod r#struct;
pub mod code;
pub mod function;
pub mod program;

pub enum TopElement {
    Struct(Struct),
    Function(Function)
}

impl DisplayIndented for TopElement {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            TopElement::Struct(class_type) => class_type.format(indent, f),
            TopElement::Function(function) => function.format(indent, f)
        }
    }
}

impl Display for TopElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

#[derive(Clone)]
pub enum Modifier {
    Public = 0b0000_0001
}

impl Display for Modifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            Modifier::Public => write!(f, "pub")
        }
    }
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
    return modifiers & target != 0;
}

fn to_modifiers(from: u8) -> Vec<Modifier> {
    let mut modifiers = Vec::new();
    if from & (Modifier::Public as u8) != 0 {
        modifiers.push(Modifier::Public)
    }
    return modifiers;
}

pub trait DisplayIndented {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result;
}