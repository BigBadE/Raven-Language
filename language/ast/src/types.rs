use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use crate::r#struct::Struct;

pub struct Types {
    pub name: String,
    pub structure: Struct,
    pub parent: Option<Rc<Types>>,
    pub traits: Vec<Rc<Types>>,
    pub is_trait: bool
}

impl Types {
    pub fn new_struct(structure: Struct, parent: Option<Rc<Types>>, traits: Vec<Rc<Types>>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent,
            traits,
            is_trait: false
        }
    }

    pub fn new_trait(structure: Struct, parent: Option<Rc<Types>>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent,
            traits: Vec::new(),
            is_trait: true
        }
    }

    pub fn is_type(&self, other: Rc<Types>) -> bool {
        let mut parent = self;
        loop {
            if parent.traits.contains(&other) {
                return true;
            }
            if parent == other.deref() {
                return true;
            }
            if let Some(next_parent) = &parent.parent {
                parent = next_parent;
            } else {
                break
            }
        }
        return false;
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