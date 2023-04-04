#![feature(box_into_inner)]
#![feature(get_mut_unchecked)]

use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::runtime::Handle;
use crate::function::Function;
use crate::r#struct::Struct;
use crate::types::Types;

pub mod async_util;
pub mod blocks;
pub mod code;
pub mod function;
pub mod r#struct;
pub mod syntax;
pub mod types;

pub static MODIFIERS: [Modifier; 5] = [Modifier::Public, Modifier::Protected, Modifier::Extern, Modifier::Internal, Modifier::Operation];

#[derive(Clone, Copy)]
pub enum Modifier {
    Public = 0b1,
    Protected = 0b10,
    Extern = 0b100,
    Internal = 0b1000,
    Operation = 0b1_0000,
    Trait = 0b1100
}

impl Display for Modifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            Modifier::Public => write!(f, "pub"),
            Modifier::Protected => write!(f, "pub(proj)"),
            Modifier::Extern => write!(f, "extern"),
            Modifier::Internal => write!(f, "internal"),
            Modifier::Operation => write!(f, "operation"),
            Modifier::Trait => panic!("Shouldn't display trait modifier!")
        }
    }
}

pub fn get_modifier(modifiers: &[Modifier]) -> u8 {
    let mut sum = 0;
    for modifier in modifiers {
        sum += modifier.clone() as u8;
    }

    return sum;
}

#[inline]
pub fn is_modifier(modifiers: u8, target: Modifier) -> bool {
    let target = target as u8;
    return modifiers & target == target as u8;
}

pub fn to_modifiers(from: u8) -> Vec<Modifier> {
    let mut modifiers = Vec::new();
    for modifier in MODIFIERS {
        if from & (modifier as u8) != 0 {
            modifiers.push(modifier)
        }
    }

    return modifiers;
}

pub trait DisplayIndented {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result;
}

#[derive(Clone)]
pub struct Attribute {
    pub value: String
}

impl Attribute {
    pub fn new(value: String) -> Self {
        return Self {
            value
        }
    }
}

/*
pub fn assign_with_priority(mut operator: Box<OperatorEffect>) -> OperatorEffect {
    //Needs ownership of the value
    let mut temp_lhs = Effects::NOP();
    mem::swap(&mut temp_lhs, operator.effects.first_mut().unwrap());
    match temp_lhs {
        // Code explained using the following example: 1 + 2 / 2
        Effects::OperatorEffect(mut lhs) => {
            // temp_lhs = (1 + 2), operator = {} / 2
            if lhs.priority < operator.priority || (!operator.parse_left && lhs.priority == operator.priority) {
                // temp_lhs = 1 + {}, operator = 2 / 2
                mem::swap(lhs.effects.last_mut().unwrap(), operator.effects.first_mut().unwrap());

                // 1 + (2 / 2)
                mem::swap(lhs.effects.last_mut().unwrap(), &mut Effects::OperatorEffect(operator));

                return Box::into_inner(lhs);
            } else {
                mem::swap(&mut Effects::OperatorEffect(lhs), operator.effects.get_mut(0).unwrap());
            }
        }
        _ => mem::swap(&mut temp_lhs, operator.effects.get_mut(0).unwrap())
    }

    return Box::into_inner(operator);
}*/

pub trait ProcessManager: Send + Sync {
    fn handle(&self) -> &Handle;

    fn add_to_next(&mut self, adding: Arc<Struct>);

    fn add_func_to_next(&mut self, adding: Arc<Function>);
}

#[derive(Clone, Debug)]
pub struct ParsingError {
    pub start: (u32, u32),
    pub end: (u32, u32),
    pub message: String
}

impl ParsingError {
    pub fn new(start: (u32, u32), end: (u32, u32), message: String) -> Self {
        return Self {
            start,
            end,
            message
        };
    }
}

impl Display for ParsingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "Error at {:?}-{:?}:\n{}", self.start, self.end, self.message);
    }
}
