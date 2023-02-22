use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::{get_modifier, is_modifier, Modifier};
use crate::code::Field;
use crate::function::Function;

pub struct Struct {
    pub modifiers: u8,
    pub members: Vec<TypeMembers>,
    pub name: Ident
}

impl Struct {
    pub fn new(members: Vec<TypeMembers>, modifiers: &[Modifier], name: Ident) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            members,
            name
        }
    }
}

impl Display for Struct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if is_modifier(self.modifiers, Modifier::Public) {
            write!(f, "pub ")?;
        }
        write!(f, "struct {} {{\n", self.name)?;
        for member in &self.members {
            write!(f, "\n{}\n", member)?;
        }
        write!(f, "}}")?;
        return Ok(());
    }
}

pub trait TypeMember: Display {

}

pub enum TypeMembers {
    Function(Function),
    Field(Field)
}

impl Display for TypeMembers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            TypeMembers::Function(function) => write!(f, "{}", function),
            TypeMembers::Field(field) => write!(f, "{}", field)
        };
    }
}