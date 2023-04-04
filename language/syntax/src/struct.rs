use std::sync::Arc;
use crate::code::MemberField;
use crate::function::Function;
use crate::ParsingError;
use crate::types::Types;

#[derive(Clone)]
pub struct Struct {
    pub modifiers: u8,
    pub name: String,
    generics: Vec<(String, Vec<Types>)>,
    pub fields: Vec<MemberField>,
    pub functions: Vec<Arc<Function>>,
    pub traits: Vec<Arc<Struct>>,
    pub poisoned: Option<ParsingError>
}

impl Struct {
    pub fn new(fields: Vec<MemberField>, generics: Vec<(String, Vec<Types>)>,
               functions: Vec<Arc<Function>>, modifiers: u8, name: String) -> Self {
        return Self {
            modifiers,
            generics,
            fields,
            functions,
            name,
            traits: Vec::new(),
            poisoned: None
        }
    }

    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            modifiers: 0,
            name,
            generics: Vec::new(),
            fields: Vec::new(),
            functions: Vec::new(),
            traits: Vec::new(),
            poisoned: Some(error)
        };
    }
}