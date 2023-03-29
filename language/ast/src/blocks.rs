use std::collections::HashMap;
use std::fmt::Formatter;
use crate::code::{AssignVariable, Effect, Effects, Expression, ExpressionType, MethodCall, VariableLoad};
use crate::DisplayIndented;
use crate::function::{Arguments, CodeBody};
use crate::type_resolver::FinalizedTypeResolver;
use crate::types::ResolvableTypes;

#[derive(Clone)]
pub struct ForStatement {
    pub variable: String,
    pub effect: Effects,
    pub code_block: CodeBody,
}

impl ForStatement {
    pub fn new(variable: String, effect: Effects, code_block: CodeBody) -> Self {
        return Self {
            variable,
            effect,
            code_block,
        };
    }
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
        self.effect = Effects::AssignVariable(Box::new(AssignVariable::new("$for".to_string(),
                                                                         self.effect.clone(), (0, 0))));
        let mut load_effect = Effects::VariableLoad(Box::new(VariableLoad::new("$for".to_string(), (0, 0))));
        self.effect.finalize(type_resolver);
        load_effect.finalize(type_resolver);
        let name = self.effect.unwrap().return_type().unwrap().unwrap().structure.functions
            .iter().find(|func: &&String| func.split("::").last().unwrap().contains("next")).unwrap().clone();
        let next = MethodCall::new(Some(load_effect.clone()),
                                   name, Arguments::new(vec!()), (0, 0));
        let mut var_set = AssignVariable::new(self.variable.clone(),
                                          Effects::MethodCall(Box::new(next)), (0, 0));
        var_set.finalize(type_resolver);
        self.code_block.expressions.insert(0, Expression::new(ExpressionType::Line,
                                                              Effects::AssignVariable(Box::new(var_set))));
        self.code_block.finalize(type_resolver);
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return None;
    }

    fn get_location(&self) -> (u32, u32) {
        panic!("Unexpected location!");
    }

    fn set_generics(&mut self, type_resolver: &mut dyn FinalizedTypeResolver, replacing: &HashMap<String, ResolvableTypes>) {
        self.effect.as_mut().set_generics(type_resolver, replacing);
        self.code_block.set_generics(type_resolver, replacing);
    }
}

#[derive(Clone)]
pub struct IfStatement {
    pub body: CodeBody,
    pub condition: Effects,
    pub else_ifs: Vec<(CodeBody, Effects)>,
    pub else_body: Option<CodeBody>,
    pub location: (u32, u32),
}

impl IfStatement {
    pub fn new(body: CodeBody, condition: Effects, location: (u32, u32)) -> Self {
        return Self {
            body,
            condition,
            else_ifs: Vec::new(),
            else_body: None,
            location,
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
        return self.location;
    }

    fn set_generics(&mut self, type_resolver: &mut dyn FinalizedTypeResolver, replacing: &HashMap<String, ResolvableTypes>) {
        self.condition.as_mut().set_generics(type_resolver, replacing);
        self.body.set_generics(type_resolver, replacing);
        if let Some(body) = &mut self.else_body {
            body.set_generics(type_resolver, replacing);
        }

        for (body, effect) in &mut self.else_ifs {
            body.set_generics(type_resolver, replacing);
            effect.as_mut().set_generics(type_resolver, replacing);
        }
    }
}

#[derive(Clone)]
pub struct SwitchStatement {
    pub value: Effects,
    pub possible: Vec<(Effects, Effects)>,
    pub location: (u32, u32),
}

impl SwitchStatement {
    pub fn new(value: Effects, possible: Vec<(Effects, Effects)>, location: (u32, u32)) -> Self {
        return Self {
            value,
            possible,
            location
        }
    }
}

impl Effect for SwitchStatement {
    fn is_return(&self) -> bool {
        for (_, found) in &self.possible {
            if !found.unwrap().is_return() {
                return false;
            }
        }
        return true;
    }

    fn has_return(&self) -> bool {
        return false;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.value.finalize(type_resolver);
        for (effect, body) in &mut self.possible {
            effect.finalize(type_resolver);
            body.finalize(type_resolver);
        }
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return self.possible.get(0).unwrap().1.unwrap().return_type();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }

    fn set_generics(&mut self, type_resolver: &mut dyn FinalizedTypeResolver, replacing: &HashMap<String, ResolvableTypes>) {
        self.value.as_mut().set_generics(type_resolver, replacing);
        for (effect, body) in &mut self.possible {
            effect.as_mut().set_generics(type_resolver, replacing);
            body.as_mut().set_generics(type_resolver, replacing);
        }
    }
}

impl DisplayIndented for SwitchStatement {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "switch ")?;
        self.value.format(parsing, f)?;
        write!(f, "{{\n")?;
        let deeper_indent = parsing.to_string() + "    ";
        for (condition, body) in &self.possible {
            write!(f, "{}", deeper_indent)?;
            condition.format(&deeper_indent, f)?;
            write!(f, " => ")?;
            body.format(&deeper_indent, f)?;
        }
        return write!(f, "}}");
    }
}