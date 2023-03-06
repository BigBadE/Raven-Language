use std::fmt::{Display, Formatter};
use crate::{DisplayIndented, get_modifier, Modifier, to_modifiers};
use crate::code::MemberField;
use crate::function::{display_joined, Function};

pub struct Struct {
    pub modifiers: u8,
    pub members: Vec<TypeMembers>,
    pub name: String
}

impl Struct {
    pub fn new(members: Vec<TypeMembers>, modifiers: &[Modifier], name: String) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            members,
            name
        }
    }
}

impl Display for Struct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl DisplayIndented for Struct {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} struct {} {{\n", display_joined(&to_modifiers(self.modifiers)), self.name)?;
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