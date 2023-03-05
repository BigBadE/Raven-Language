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
        write!(f, "for {} in {} ", self.variable, self.effect)?;
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