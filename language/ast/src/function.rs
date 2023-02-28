use std::fmt::{Display, Formatter};
use crate::r#struct::TypeMember;
use crate::code::{Effects, Expression, Field};
use crate::{DisplayIndented, get_modifier, Modifier, to_modifiers};

pub struct Function {
    pub modifiers: u8,
    pub fields: Vec<Field>,
    pub code: CodeBody,
    pub return_type: Option<String>,
    pub name: String
}

impl Function {
    pub fn new(modifiers: &[Modifier], fields: Vec<Field>, code: CodeBody, return_type: Option<String>, name: String) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            fields,
            code,
            return_type,
            name
        };
    }
}

#[derive(Default)]
pub struct Arguments {
    pub arguments: Vec<Effects>,
}

impl Arguments {
    pub fn new(arguments: Vec<Effects>) -> Self {
        return Self {
            arguments
        };
    }
}

#[derive(Default)]
pub struct CodeBody {
    pub expressions: Vec<Expression>
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>) -> Self {
        return Self {
            expressions
        };
    }

    pub fn is_return(&self) -> bool {
        return self.expressions.iter().find(|expression| expression.is_return()).is_some()
    }
}

impl DisplayIndented for Function {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{} fn {}{} ", indent, display(&to_modifiers(self.modifiers)), self.name, display(&self.fields))?;
        if self.return_type.is_some() {
            write!(f, "-> {} ", self.return_type.as_ref().unwrap())?;
        }
        return self.code.format((indent.to_string() + "    ").as_str(), f);
    }
}

impl DisplayIndented for CodeBody {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\n")?;
        for expression in &self.expressions {
            expression.format(indent, f)?;
        }
        write!(f, "}}\n")?;
        return Ok(());
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", display(&self.arguments));
    }
}

pub fn display<T>(input: &Vec<T>) -> String where T : Display {
    if input.is_empty() {
        return "()".to_string();
    }

    let mut output = String::new();
    for element in input {
        output += &*format!("{}, ", element);
    }
    return (&output[..output.len() - 2]).to_string();
}

impl TypeMember for Function {}