use std::fmt::Formatter;
use crate::code::{Effect, Effects};
use crate::DisplayIndented;
use crate::function::CodeBody;
use crate::type_resolver::TypeResolver;
use crate::types::Types;

pub struct ForStatement<'a> {
    pub variable: String,
    pub effect: Effects<'a>,
    pub code_block: CodeBody<'a>
}

impl<'a> DisplayIndented for ForStatement<'a> {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "for {} in {} ", self.variable, self.effect)?;
        let indent = indent.to_string() + "    ";
        return self.code_block.format(indent.as_str(), f);
    }
}

impl<'a> Effect<'a> for ForStatement<'a> {
    fn is_return(&self) -> bool {
        for expression in &self.code_block.expressions {
            if expression.effect.unwrap().is_return() {
                return true;
            }
        }
        return false;
    }

    fn return_type(&'a self, _type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>> {
        todo!()
    }

    fn get_location(&self) -> (u32, u32) {
        panic!("Unexpected location!");
    }
}