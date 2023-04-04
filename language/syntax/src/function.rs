use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use crate::{Attribute, DisplayIndented, ParsingError, to_modifiers, Types};
use crate::code::{Expression, Field};

pub struct Function {
    pub attributes: HashMap<String, Attribute>,
    pub generics: HashMap<String, Vec<Types>>,
    pub modifiers: u8,
    pub fields: Vec<Field>,
    pub code: CodeBody,
    pub return_type: Option<Types>,
    pub name: String,
    pub poisoned: Option<ParsingError>
}

impl Function {
    pub fn new(attributes: HashMap<String, Attribute>, modifiers: u8, fields: Vec<Field>, generics: HashMap<String, Vec<Types>>,
               code: CodeBody, return_type: Option<Types>, name: String) -> Self {
        return Self {
            attributes,
            generics,
            modifiers,
            fields,
            code,
            return_type,
            name,
            poisoned: None
        };
    }
    
    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: HashMap::new(),
            generics: HashMap::new(),
            modifiers: 0,
            fields: Vec::new(),
            code: CodeBody::new(Vec::new(), "poison".to_string()),
            return_type: None,
            name,
            poisoned: Some(error)
        }
    }
}

#[derive(Clone, Default)]
pub struct CodeBody {
    pub label: String,
    pub expressions: Vec<Expression>,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self {
            label,
            expressions
        };
    }
}

impl Debug for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl DisplayIndented for Function {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{} fn {}", indent, display_joined(&to_modifiers(self.modifiers)),
               self.name)?;

        if !self.generics.is_empty() {
            write!(f, "<")?;
            for (name, bounds) in &self.generics {
                write!(f, "{}", name)?;
                if !bounds.is_empty() {
                    write!(f, ": {}", display(bounds, " + "))?;
                }
            }
            write!(f, ">")?;
        }

        write!(f, "{} ", display(&self.fields, ", "))?;

        if self.return_type.is_some() {
            write!(f, "-> {} ", self.return_type.as_ref().unwrap().name)?;
        }
        return self.code.format(indent, f);
    }
}

impl DisplayIndented for CodeBody {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {{\n", self.label)?;
        let deeper_indent = indent.to_string() + "    ";
        for expression in &self.expressions {
            expression.format(deeper_indent.as_str(), f)?;
        }
        write!(f, "{}}}", indent)?;
        return Ok(());
    }
}

pub fn display_joined<T>(input: &Vec<T>) -> String where T: Display {
    if input.is_empty() {
        return String::new();
    }
    let mut output = String::new();
    for element in input {
        output += &*format!("{} ", element);
    }
    return output[..output.len()-1].to_string();
}

pub fn display<T>(input: &Vec<T>, deliminator: &str) -> String where T: Display {
    if input.is_empty() {
        return "()".to_string();
    }

    let mut output = String::new();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return format!("({})", (&output[..output.len() - deliminator.len()]).to_string());
}

pub fn display_parenless<T>(input: &Vec<T>, deliminator: &str) -> String where T: Display {
    if input.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return (&output[..output.len() - deliminator.len()]).to_string();
}