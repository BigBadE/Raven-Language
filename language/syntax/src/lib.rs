#![feature(box_into_inner)]
#![feature(get_mut_unchecked)]

use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use async_trait::async_trait;
use crate::async_getters::AsyncGetter;
use crate::function::Function;
use crate::r#struct::Struct;
use crate::syntax::Syntax;
use crate::types::Types;

pub mod async_getters;
pub mod async_util;
pub mod blocks;
pub mod code;
pub mod function;
pub mod r#struct;
pub mod syntax;
pub mod types;

pub type ParsingFuture<T> = Pin<Box<dyn Future<Output=Result<T, ParsingError>> + Send + Sync>>;

pub static MODIFIERS: [Modifier; 5] = [Modifier::Public, Modifier::Protected, Modifier::Extern, Modifier::Internal, Modifier::Operation];

#[derive(Clone, Copy, PartialEq)]
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
pub enum Attribute {
    Basic(String),
    Integer(String, i64),
    Bool(String, bool),
    String(String, String)
}

impl Attribute {
    pub fn find_attribute<'a>(name: &str, attributes: &'a Vec<Attribute>) -> Option<&'a Attribute> {
        for attribute in attributes {
            if match attribute {
                Attribute::Basic(found) => found == name,
                Attribute::Integer(found, _) => found == name,
                Attribute::Bool(found, _) => found == name,
                Attribute::String(found, _) => found == name
            } {
                return Some(attribute);
            }
        }
        return None;
    }
}

#[async_trait]
pub trait ProcessManager: Send + Sync {
    fn handle(&self) -> &Handle;

    async fn verify_func(&self, function: &mut Function, syntax: &Arc<Mutex<Syntax>>);

    async fn verify_struct(&self, structure: &mut Struct, syntax: &Arc<Mutex<Syntax>>);

    fn add_implementation(&self, base: Types, implementing: Types);

    fn get_internal(&self, name: &str) -> Arc<Struct>;

    fn cloned(&self) -> Box<dyn ProcessManager>;

    fn init(&mut self, syntax: Arc<Mutex<Syntax>>);
}

#[derive(Clone, Debug)]
pub struct ParsingError {
    pub file: String,
    pub start: (u32, u32),
    pub start_offset: usize,
    pub end: (u32, u32),
    pub end_offset: usize,
    pub message: String
}

impl ParsingError {
    pub fn empty() -> Self {
        return ParsingError {
            file: String::new(),
            start: (0, 0),
            start_offset: 0,
            end: (0, 0),
            end_offset: 0,
            message: "You shouldn't see this!".to_string()
        }
    }

    pub fn new(file: String, start: (u32, u32), start_offset: usize, end: (u32, u32),
        end_offset: usize, message: String) -> Self {
        return Self {
            file,
            start,
            start_offset,
            end,
            end_offset,
            message
        };
    }
}

pub trait ErrorProvider {
    fn get_error(&self, error: String) -> ParsingError;
}

impl Display for ParsingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "Error at {} ({}:{}):\n{}", self.file, self.start.0, self.start.1, self.message);
    }
}

pub trait VariableManager {
    fn get_variable(&self, name: &String) -> Option<Types>;
}

#[async_trait]
pub trait TopElement where Self: Sized {
    fn poison(&mut self, error: ParsingError);

    fn is_operator(&self) -> bool;
    
    fn errors(&self) -> &Vec<ParsingError>;

    fn name(&self) -> &String;

    fn new_poisoned(name: String, error: ParsingError) -> Self;

    async fn verify(&mut self, syntax: &Arc<Mutex<Syntax>>, process_manager: &mut dyn ProcessManager);

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self>;
}