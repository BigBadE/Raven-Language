use std::fmt::{Display, Formatter};
use crate::{DisplayIndented, to_modifiers};
use crate::code::MemberField;
use crate::function::{display_joined};
use crate::type_resolver::FinalizedTypeResolver;

pub struct Struct {
    pub modifiers: u8,
    pub generics: Vec<(String, Vec<String>)>,
    pub fields: Option<Vec<MemberField>>,
    pub functions: Vec<String>,
    pub name: String
}

impl Struct {
    pub fn new(fields: Option<Vec<MemberField>>, generics: Vec<(String, Vec<String>)>,
               functions: Vec<String>, modifiers: u8, name: String) -> Self {
        return Self {
            modifiers,
            generics,
            fields,
            functions,
            name
        }
    }

    pub fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        if self.fields.is_some() {
            for field in self.fields.as_mut().unwrap() {
                field.field.finalize(type_resolver);
            }
        }
    }

    pub fn format(&self, indent: &str, f: &mut Formatter<'_>, type_manager: &dyn FinalizedTypeResolver) -> std::fmt::Result {
        write!(f, "{} struct {} {{", display_joined(&to_modifiers(self.modifiers)), self.name)?;
        let deeper_indent = "    ".to_string() + indent;
        let deeper_indent = deeper_indent.as_str();

        if self.fields.is_some() {
            for field in self.fields.as_ref().unwrap() {
                write!(f, "\n")?;
                DisplayIndented::format(field, deeper_indent, f)?;
            }
        }

        write!(f, "\n")?;
        for member in &self.functions {
            write!(f, "\n")?;
            DisplayIndented::format(type_manager.get_function(member).unwrap(), deeper_indent, f)?;
            write!(f, "\n")?;
        }
        write!(f, "{}}}", indent)?;
        return Ok(());
    }
}