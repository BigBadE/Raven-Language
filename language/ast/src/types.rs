use std::collections::HashMap;
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

    pub fn set_generic(&mut self, type_resolver: &mut dyn FinalizedTypeResolver, resolving: &HashMap<String, ResolvableTypes>) {
        if let ResolvableTypes::Resolving(name) = self {
            match resolving.get(name) {
                Some(found) => {
                    *self = found.clone()
                },
                None => {
                    match type_resolver.get_generic_struct(&name.split("<").next().unwrap().to_string()) {
                        Some(generic_struct) => {
                            self.finalize(type_resolver);
                        },
                        None => {}
                    }
                }
            }
        }
    }

    pub fn unwrap(&self) -> &Rc<Types> {
        match self { 
            ResolvableTypes::Resolved(types) => return types,
            ResolvableTypes::Resolving(name) => panic!("Expected {} to be resolved!", name),
        }
    }

    pub fn name(&self) -> &String {
        return match self {
            ResolvableTypes::Resolving(found) => found,
            ResolvableTypes::Resolved(found) => &found.name,
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

impl Debug for ResolvableTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return Display::fmt(self, f);
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

    pub fn new_trait(pointer_size: u32, structure: Struct, parent: Vec<ResolvableTypes>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent: None,
            traits: parent,
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

    pub fn is_type(&self, other: &Rc<Types>) -> bool {
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

    pub fn has_parent(&self, other: &Rc<Types>) -> bool {
        let mut parent = &self.parent;
        while let Some(found_parent) = parent {
            if found_parent.unwrap() == other {
                return true;
            }
            parent = &found_parent.unwrap().parent;
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