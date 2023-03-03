use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use crate::r#struct::Struct;

pub struct Types<'a> {
    pub name: String,
    pub structure: Struct<'a>,
    pub parent: Option<&'a Types<'a>>,
    pub traits: Vec<Types<'a>>,
    pub is_trait: bool
}

impl<'a> Types<'a> {
    pub fn new_struct(structure: Struct<'a>, parent: Option<&'a Types<'a>>, traits: Vec<Types<'a>>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent,
            traits,
            is_trait: false
        }
    }

    pub fn new_trait(structure: Struct<'a>, parent: Option<&'a Types<'a>>) -> Self {
        return Self {
            name: structure.name.clone(),
            structure,
            parent,
            traits: Vec::new(),
            is_trait: true
        }
    }

    pub fn is_type(&self, other: Types) -> bool {
        let mut parent = self;
        loop {
            if parent.traits.contains(&other) {
                return true;
            }
            if parent == &other {
                return true;
            }
            if let Some(next_parent) = parent.parent {
                parent = next_parent;
            } else {
                break
            }
        }
        return false;
    }
}

impl<'a> Hash for Types<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl<'a> Display for Types<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
    }
}

impl<'a> Eq for Types<'a> {}

impl<'a> PartialEq for Types<'a> {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name;
    }
}