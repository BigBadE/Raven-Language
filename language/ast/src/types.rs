use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use crate::code::MemberField;
use crate::r#struct::Struct;
use crate::type_resolver::FinalizedTypeResolver;

#[derive(Clone, PartialEq, Eq)]
pub enum ResolvableTypes {
    Resolved(Rc<Types>),
    Resolving(String)
}

impl ResolvableTypes {
    pub fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        type_resolver.finalize(self);
    }
    
    pub fn unwrap(&self) -> &Rc<Types> {
        match self { 
            ResolvableTypes::Resolved(types) => return types,
            ResolvableTypes::Resolving(_) => panic!("Expected resolved type!")
        }
    }
}

impl Display for ResolvableTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvableTypes::Resolving(resolving) => write!(f, "{}", resolving),
            ResolvableTypes::Resolved(resolved) => write!(f, "{}", resolved)
        }
    }
}

pub struct Types {
    pub name: String,
    pub structure: Struct,
    pub parent: Option<ResolvableTypes>,
    pub traits: Vec<ResolvableTypes>,
    pub size: u32,
    pub is_trait: bool
}

impl Types {
    pub fn new_struct(structure: Struct, parent: Option<ResolvableTypes>, traits: Vec<ResolvableTypes>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent,
            traits,
            size: 0,
            is_trait: false
        }
    }

    pub fn new_trait(pointer_size: u32, structure: Struct, parent: Option<ResolvableTypes>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent,
            traits: Vec::new(),
            is_trait: true,
            size: pointer_size
        }
    }

    pub fn get_fields(&self) -> &Vec<MemberField> {
        let mut parent = self;
        loop {
            if parent.structure.fields.is_some() {
                return parent.structure.fields.as_ref().unwrap();
            }
            parent = parent.parent.as_ref().unwrap().unwrap().as_ref();
        }
    }

    pub fn is_type(&self, other: Rc<Types>) -> bool {
        let mut parent = self;
        loop {
            if parent.traits.contains(&ResolvableTypes::Resolved(other.clone())) {
                return true;
            }
            if parent == other.deref() {
                return true;
            }
            if let Some(next_parent) = &parent.parent {
                parent = next_parent.unwrap();
            } else {
                break
            }
        }
        return false;
    }
}

impl Debug for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return Display::fmt(self, f);
    }
}

impl Hash for Types {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
    }
}

impl Eq for Types {}

impl PartialEq for Types {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name;
    }
}