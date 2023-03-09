use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use crate::r#struct::Struct;
use crate::type_resolver::TypeResolver;

pub enum ResolvableTypes {
    Resolved(Rc<Types>),
    Resolving(UnresolvedType)
}

impl ResolvableTypes {
    pub fn finalize(&mut self, type_resolver: &dyn TypeResolver) {
        match self {
            ResolvableTypes::Resolving(resolving) => 
                *self = ResolvableTypes::Resolved(type_resolver.get_type(&resolving.name).unwrap()),
            ResolvableTypes::Resolved(_) => panic!("Tried to resolve already-resolved type!")
        }
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
            ResolvableTypes::Resolving(resolving) => write!(f, "{}", resolving.name),
            ResolvableTypes::Resolved(resolved) => write!(f, "{}", resolved)
        }
    }
}

pub struct UnresolvedType {
    pub name: String
}

impl UnresolvedType {
    pub fn new(name: String) -> Self {
        return Self {
            name
        }
    }
}
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