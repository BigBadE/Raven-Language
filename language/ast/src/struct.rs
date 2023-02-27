use std::fmt::Formatter;
use crate::basic_types::Ident;
use crate::{DisplayIndented, get_modifier, is_modifier, Modifier};
use crate::code::MemberField;
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

impl DisplayIndented for Struct {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        if is_modifier(self.modifiers, Modifier::Public) {
            write!(f, "pub ")?;
        }
        write!(f, "struct {} {{\n", self.name)?;
        for member in &self.members {
            write!(f, "\n")?;
            DisplayIndented::format(member, indent, f)?;
            write!(f, "\n")?;
        }
        write!(f, "}}")?;
        return Ok(());
    }
}

pub trait TypeMember: DisplayIndented {

}

pub enum TypeMembers {
    Function(Function),
    Field(MemberField)
}

impl DisplayIndented for TypeMembers {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let indent = indent.to_string() + "    ";
        let indent = indent.as_str();
        return match self {
            TypeMembers::Function(function) => DisplayIndented::format(function, indent, f),
            TypeMembers::Field(field) => DisplayIndented::format(field, indent, f)
        };
    }
}