use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::r#struct::TypeMember;
use crate::code::{Expression, Field};
use crate::{get_modifier, is_modifier, Modifier, to_modifiers};

pub struct Function {
    pub modifiers: u8,
    pub fields: Vec<Field>,
    pub code: CodeBody,
    pub name: Ident,
}

impl Function {
    pub fn new(modifiers: &[Modifier], fields: Vec<Field>, code: CodeBody, name: Ident) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            fields,
            code,
            name,
        };
    }
}

#[derive(Default)]
pub struct Arguments {
    pub arguments: Vec<Expression>,
}

impl Arguments {
    pub fn new(arguments: Vec<Expression>) -> Self {
        return Self {
            arguments
        };
    }
}

#[derive(Default)]
pub struct CodeBody {
    pub expressions: Vec<Expression>,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>) -> Self {
        return Self {
            expressions
        };
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} fn {}({}) {}", display(&to_modifiers(self.modifiers)), self.name, display(&self.fields), self.code)?;
        return Ok(());
    }
}

impl Display for CodeBody {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\n")?;
        for expression in &self.expressions {
            Display::fmt(expression, f)?;
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

fn display<T>(input: &Vec<T>) -> String where T : Display {
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