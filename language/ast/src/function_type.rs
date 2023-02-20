use std::fmt::{Display, Formatter};
use crate::basic_types::Ident;
use crate::class_type::TypeMember;
use crate::code::Expression;
use crate::{get_modifier, is_modifier, Modifier, TopElement};

pub struct Function {
    pub modifiers: u8,
    pub code: CodeBody,
    pub name: Ident
}

impl Function {
    pub fn new(modifiers: &[Modifier], code: CodeBody, name: Ident) -> Self {
        return Self {
            modifiers: get_modifier(modifiers),
            code,
            name
        }
    }
}

pub struct CodeBody {
    pub expressions: Vec<Expression>
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>) -> Self {
        return Self {
            expressions
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if is_modifier(self.modifiers, Modifier::Public) {
            write!(f, "pub ")?;
        }
        write!(f, "fn {} {}", self.name, self.code)?;
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

impl TypeMember for Function {

}