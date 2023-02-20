use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::{get_modifier, is_modifier, Modifier, TopElement};

pub struct ClassType {
    pub modifiers: u8,
    pub members: Vec<Box<dyn TypeMember>>,
    pub name: Ident
}

impl ClassType {
    pub fn new(members: Vec<Box<dyn TypeMember>>, modifiers: &[Modifier], name: Ident) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            members,
            name
        }
    }
}

impl Display for ClassType {
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