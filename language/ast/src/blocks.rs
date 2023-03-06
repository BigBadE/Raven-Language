use std::fmt::Formatter;
use std::rc::Rc;
use crate::code::{Effect, Effects};
use crate::DisplayIndented;
use crate::function::CodeBody;
use crate::type_resolver::TypeResolver;
use crate::types::Types;

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
        let indent = indent.to_string() + "    ";
        return self.code_block.format(indent.as_str(), f);
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

    fn return_type(&self, _type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
        todo!()
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
        let deeper_indent = indent.to_string() + "    ";
        let deeper_indent = deeper_indent.as_str();
        self.body.format(deeper_indent, f)?;

        for (effect, body) in &self.else_ifs {
            write!(f, " else if ")?;
            effect.format(indent, f)?;
            write!(f, " ")?;
            body.format(deeper_indent, f)?;
        }

        if self.else_body.is_some() {
            write!(f, " else ")?;
            self.else_body.as_ref().unwrap().format(deeper_indent, f)?;
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

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
        return self.body.return_type(type_resolver);
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}
