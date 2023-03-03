use std::fmt::{Display, Formatter};
use crate::{DisplayIndented, get_modifier, is_modifier, Modifier};
use crate::code::MemberField;
use crate::function::Function;

pub struct Struct<'a> {
    pub modifiers: u8,
    pub members: Vec<TypeMembers<'a>>,
    pub name: String
}

impl<'a> Struct<'a> {
    pub fn new(members: Vec<TypeMembers<'a>>, modifiers: &[Modifier], name: String) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            members,
            name
        }
    }
}

impl<'a> Display for Struct<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl<'a> DisplayIndented for Struct<'a> {
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

pub enum TypeMembers<'a> {
    Function(Function<'a>),
    Field(MemberField<'a>)
}

impl<'a> DisplayIndented for TypeMembers<'a> {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let indent = indent.to_string() + "    ";
        let indent = indent.as_str();
        return match self {
            TypeMembers::Function(function) => DisplayIndented::format(function, indent, f),
            TypeMembers::Field(field) => DisplayIndented::format(field, indent, f)
        };
    }
}