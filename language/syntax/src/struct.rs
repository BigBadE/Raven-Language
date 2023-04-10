use std::collections::HashMap;
use std::sync::Arc;
use crate::code::MemberField;
use crate::function::Function;
use crate::{Attribute, ParsingError};
use crate::types::Types;

#[derive(Clone)]
pub struct Struct {
    pub modifiers: u8,
    pub name: String,
    generics: HashMap<String, Types>,
    pub attributes: Vec<Attribute>,
    pub fields: Vec<MemberField>,
    pub functions: Vec<Arc<Function>>,
    pub traits: Vec<Arc<Struct>>,
    pub poisoned: Vec<ParsingError>
}

impl Struct {
    pub fn new(attributes: Vec<Attribute>, fields: Vec<MemberField>, generics: HashMap<String, Types>,
               functions: Vec<Arc<Function>>, modifiers: u8, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            generics,
            fields,
            functions,
            name,
            traits: Vec::new(),
            poisoned: Vec::new()
        }
    }

    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            modifiers: 0,
            name,
            generics: HashMap::new(),
            fields: Vec::new(),
            functions: Vec::new(),
            traits: Vec::new(),
            poisoned: vec!(error)
        };
    }
}