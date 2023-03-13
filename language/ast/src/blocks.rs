use std::fmt::Formatter;
use crate::code::{Effect, Effects};
use crate::DisplayIndented;
use crate::function::CodeBody;
use crate::type_resolver::FinalizedTypeResolver;
use crate::types::ResolvableTypes;

pub struct ForStatement {
    pub variable: String,
    pub effect: Effects,
    pub code_block: CodeBody
}

impl DisplayIndented for ForStatement {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "for {} in ", self.variable)?;
        self.effect.format(indent, f)?;
        write!(f, " ")?;
        return self.code_block.format(indent, f);
    }
}

impl Effect for ForStatement {
    fn is_return(&self) -> bool {
        for expression in &self.code_block.expressions {
            if expression.effect.unwrap().is_return() {
                return true;
            }
        }
        return false;
    }

    fn has_return(&self) -> bool {
        return false;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.code_block.finalize(type_resolver);
        self.effect.finalize(type_resolver);
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return None;
    }

    fn get_location(&self) -> (u32, u32) {
        panic!("Unexpected location!");
    }
}

pub struct IfStatement {
    pub body: CodeBody,
    pub condition: Effects,
    pub else_ifs: Vec<(CodeBody, Effects)>,
    pub else_body: Option<CodeBody>,
    pub location: (u32, u32)
}

impl IfStatement {
    pub fn new(body: CodeBody, condition: Effects, location: (u32, u32)) -> Self {
        return Self {
            body,
            condition,
            else_ifs: Vec::new(),
            else_body: None,
            location
        };
    }
}

impl DisplayIndented for IfStatement {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "if ")?;
        self.condition.format(indent, f)?;
        write!(f, " ")?;
        self.body.format(indent, f)?;

        for (effect, body) in &self.else_ifs {
            write!(f, " else if ")?;
            effect.format(indent, f)?;
            write!(f, " ")?;
            body.format(indent, f)?;
        }

        if self.else_body.is_some() {
            write!(f, " else ")?;
            self.else_body.as_ref().unwrap().format(indent, f)?;
        }
        return Ok(());
    }
}

impl Effect for IfStatement {
    fn is_return(&self) -> bool {
        if !self.body.is_return() {
            return false;
        }
        if self.else_body.is_some() && !self.else_body.as_ref().unwrap().is_return() {
            return false;
        }

        for (body, _) in &self.else_ifs {
            if !body.is_return() {
                return false;
            }
        }

        return true;
    }

    fn has_return(&self) -> bool {
        return false;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.body.finalize(type_resolver);
        if self.else_body.is_some() {
            self.else_body.as_mut().unwrap().finalize(type_resolver);
        }
        self.condition.finalize(type_resolver);
        for (else_if, condition) in &mut self.else_ifs {
            else_if.finalize(type_resolver);
            condition.finalize(type_resolver);
        }
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return self.body.return_type();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}
