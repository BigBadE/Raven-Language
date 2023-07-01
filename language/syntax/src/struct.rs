use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use indexmap::map::IndexMap;
use lazy_static::lazy_static;
use async_trait::async_trait;
use crate::{AsyncGetter, Modifier, ParsingFuture, ProcessManager, Syntax, TopElement};
use crate::code::{FinalizedMemberField, MemberField};
use crate::{Attribute, ParsingError};
use crate::async_util::NameResolver;
use crate::types::{FinalizedTypes, Types};

lazy_static! {
pub static ref I64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "i64".to_string())));
pub static ref F64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "f64".to_string())));
pub static ref U64: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "u64".to_string())));
pub static ref STR: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "str".to_string())));
pub static ref BOOL: Arc<FinalizedStruct> = Arc::new(FinalizedStruct::empty_of(StructData::new(Vec::new(), Modifier::Internal as u8, "bool".to_string())));
}

pub static ID: Mutex<u64> = Mutex::new(0);

#[derive(Clone)]
pub struct StructData {
    pub modifiers: u8,
    pub id: u64,
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub traits: Vec<Arc<StructData>>,
    pub poisoned: Vec<ParsingError>,
}

pub struct UnfinalizedStruct {
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub fields: Vec<ParsingFuture<MemberField>>,
    pub data: Arc<StructData>,
}

#[derive(Clone, Debug)]
pub struct FinalizedStruct {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub data: Arc<StructData>,
}

impl Hash for StructData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.id.to_be_bytes())
    }
}

impl PartialEq for StructData {
    fn eq(&self, other: &Self) -> bool {
        return self.id == other.id;
    }
}

impl PartialEq for FinalizedStruct {
    fn eq(&self, other: &Self) -> bool {
        return self.data.id == other.data.id;
    }
}

impl StructData {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String) -> Self {
        let mut id = ID.lock().unwrap();
        *id += 1;
        return Self {
            attributes,
            id: *id,
            modifiers,
            name,
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
            traits: Vec::new(),
            poisoned: vec!(error),
        };
    }
}

impl FinalizedStruct {
    pub fn empty_of(data: StructData) -> Self {
        return Self {
            generics: IndexMap::new(),
            fields: Vec::new(),
            data: Arc::new(data),
        };
    }

    pub async fn degeneric(&mut self, generics: &Vec<FinalizedTypes>, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
        let mut i = 0;
        for value in self.generics.values_mut() {
            for generic in value {
                if let FinalizedTypes::Generic(name, bounds) = generic {
                    let name = name.clone();
                    let temp: &FinalizedTypes = generics.get(i).unwrap();
                    for bound in bounds {
                        if !temp.of_type(&bound, syntax).await {
                            panic!("Generic {} set to a {} which isn't a {}", name, temp.name(), bound.name());
                        }
                    }
                    *generic = temp.clone();
                    i += 1;
                } else {
                    panic!("Guhh?????");
                }
            }
        }

        for field in &mut self.fields {
            let types = &mut field.field.field_type;
            if let FinalizedTypes::Generic(name, _) = types {
                let index = self.generics.iter().position(|(other_name, _)| name == other_name).unwrap();
                let generic: &FinalizedTypes = generics.get(index).unwrap();
                *types = generic.clone();
            }
        }

        return Ok(());
    }
}

#[async_trait]
impl TopElement<FinalizedStruct> for StructData {
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
        return StructData::new_poisoned(name, error);
    }

    async fn verify(mut current: Arc<Self>, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        process_manager.verify_struct(unsafe { Arc::get_mut_unchecked(&mut current) }, resolver, syntax).await;
    }

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self, FinalizedStruct> {
        return &mut syntax.structures;
    }
}

impl Debug for StructData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Eq for StructData {}

impl Eq for FinalizedStruct {}