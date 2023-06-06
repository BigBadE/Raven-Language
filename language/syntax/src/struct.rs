use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use indexmap::map::IndexMap;
use lazy_static::lazy_static;
use async_trait::async_trait;
use crate::{AsyncGetter, Modifier, ProcessManager, Syntax, TopElement};
use crate::code::MemberField;
use crate::{Attribute, ParsingError};
use crate::async_util::NameResolver;
use crate::syntax::ParsingType;
use crate::types::Types;

lazy_static! {
pub static ref I64: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), IndexMap::new(),
        Modifier::Internal as u8, "i64".to_string()));
pub static ref F64: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), IndexMap::new(),
        Modifier::Internal as u8, "f64".to_string()));
pub static ref U64: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), IndexMap::new(),
        Modifier::Internal as u8, "u64".to_string()));
pub static ref STR: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), IndexMap::new(),
        Modifier::Internal as u8, "str".to_string()));
pub static ref BOOL: Arc<Struct> = Arc::new(Struct::new(Vec::new(), Vec::new(), IndexMap::new(),
        Modifier::Internal as u8, "bool".to_string()));
}

pub static ID: Mutex<u64> = Mutex::new(0);

#[derive(Clone)]
pub struct Struct {
    pub modifiers: u8,
    pub id: u64,
    pub name: String,
    pub generics: IndexMap<String, ParsingType<Types>>,
    pub attributes: Vec<Attribute>,
    pub fields: Vec<ParsingType<MemberField>>,
    pub traits: Vec<Arc<Struct>>,
    pub poisoned: Vec<ParsingError>,
}

impl Hash for Struct {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.id.to_be_bytes())
    }
}

impl PartialEq for Struct {
    fn eq(&self, other: &Self) -> bool {
        return self.id == other.id;
    }
}

impl Struct {
    pub fn new(attributes: Vec<Attribute>, fields: Vec<ParsingType<MemberField>>, generics: IndexMap<String, ParsingType<Types>>,
               modifiers: u8, name: String) -> Self {
        let mut id = ID.lock().unwrap();
        *id += 1;
        return Self {
            attributes,
            id: *id,
            modifiers,
            fields,
            name,
            generics,
            traits: Vec::new(),
            poisoned: Vec::new(),
        };
    }

    pub fn new_poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            id: 0,
            modifiers: 0,
            name,
            generics: IndexMap::new(),
            fields: Vec::new(),
            traits: Vec::new(),
            poisoned: vec!(error),
        };
    }

    pub async fn degeneric(&mut self, generics: &Vec<Types>, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
        let mut i = 0;
        for value in self.generics.values_mut() {
            if let Types::Generic(name, bounds) = value.await_finish().await? {
                let name = name.clone();
                let temp = generics.get(i).unwrap().clone();
                for bound in bounds {
                    if !temp.of_type(&bound, syntax).await {
                        panic!("Generic {} set to a {} which isn't a {}", name, temp, bound);
                    }
                }
                *value = ParsingType::Done(temp);
                i += 1;
            } else {
                panic!("Guhh?????");
            }
        }

        for field in &mut self.fields {
            let types = &mut field.assume_finished_mut().field.field_type;
            if let Types::Generic(name, _) = types {
                *types = self.generics.get(name).unwrap().assume_finished().clone();
            }
        }

        return Ok(());
    }
}

#[async_trait]
impl TopElement for Struct {
    fn poison(&mut self, error: ParsingError) {
        self.poisoned.push(error);
    }

    fn is_operator(&self) -> bool {
        return false;
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

    async fn verify(mut current: Arc<Self>, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        process_manager.verify_struct(unsafe { Arc::get_mut_unchecked(&mut current) }, resolver, syntax).await;
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

impl Eq for Struct {}