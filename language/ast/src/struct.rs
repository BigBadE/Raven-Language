use std::fmt::{Display, Formatter};
use crate::{DisplayIndented, to_modifiers};
use crate::code::MemberField;
use crate::function::{display_joined, Function};
use crate::type_resolver::FinalizedTypeResolver;
use crate::types::ResolvableTypes;

pub struct Struct {
    pub modifiers: u8,
    pub generics: Vec<ResolvableTypes>,
    pub fields: Option<Vec<MemberField>>,
    pub functions: Vec<Function>,
    pub name: String
}

impl Struct {
    pub fn new(fields: Option<Vec<MemberField>>, generics: Vec<ResolvableTypes>, 
               functions: Vec<Function>, modifiers: u8, name: String) -> Self {
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

        for generic in &mut self.generics {
            generic.finalize(type_resolver);
        }
        
        for function in &mut self.functions {
            function.finalize(type_resolver);
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
            DisplayIndented::format(member, deeper_indent, f)?;
            write!(f, "\n")?;
        }
        write!(f, "{}}}", indent)?;
        return Ok(());
    }
}