use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use async_trait::async_trait;
use crate::{AsyncGetter, Modifier, ProcessManager, Syntax, TopElement};
use crate::code::MemberField;
use crate::function::Function;
use crate::{Attribute, ParsingError};
use crate::types::Types;

lazy_static! {
pub static ref I64: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), HashMap::new(), Vec::new(),
        Modifier::Internal as u8, "i64".to_string()));
pub static ref F64: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), HashMap::new(), Vec::new(),
        Modifier::Internal as u8, "f64".to_string()));
pub static ref U64: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), HashMap::new(), Vec::new(),
        Modifier::Internal as u8, "u64".to_string()));
pub static ref STR: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), HashMap::new(), Vec::new(),
        Modifier::Internal as u8, "str".to_string()));
}

#[derive(Clone)]
pub struct Struct {
    pub modifiers: u8,
    pub name: String,
    _generics: HashMap<String, Types>,
    pub attributes: Vec<Attribute>,
    pub fields: Vec<MemberField>,
    pub functions: Vec<Arc<Function>>,
    pub traits: Vec<Arc<Struct>>,
    pub poisoned: Vec<ParsingError>,
}

impl Struct {
    pub fn new(attributes: Vec<Attribute>, fields: Vec<MemberField>, generics: HashMap<String, Types>,
               functions: Vec<Arc<Function>>, modifiers: u8, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            _generics: generics,
            fields,
            functions,
            name,
            traits: Vec::new(),
            poisoned: Vec::new(),
        };
    }

    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            modifiers: 0,
            name,
            _generics: HashMap::new(),
            fields: Vec::new(),
            functions: Vec::new(),
            traits: Vec::new(),
            poisoned: vec!(error),
        };
    }
}

#[async_trait]
impl TopElement for Struct {
    fn poison(&mut self, error: ParsingError) {
        self.poisoned.push(error);
    }

    fn errors(&self) -> &Vec<ParsingError> {
        return &self.poisoned;
    }

    fn name(&self) -> &String {
        return &self.name;
    }

    fn new_poisoned(name: String, error: ParsingError) -> Self {
        return Struct::new_poisoned(name, error);
    }

    async fn verify(&mut self, syntax: &Arc<Mutex<Syntax>>, process_manager: &mut dyn ProcessManager) {
        process_manager.verify_struct(self, syntax).await;
    }

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self> {
        return &mut syntax.structures;
    }
}

impl Debug for Struct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl PartialEq for Struct {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name;
    }
}