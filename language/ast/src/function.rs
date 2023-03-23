use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use crate::code::{Effect, Effects, Expression, ExpressionType, Field};
use crate::{Attribute, DisplayIndented, to_modifiers};
use crate::type_resolver::FinalizedTypeResolver;
use crate::types::ResolvableTypes;

pub struct Function {
    pub attributes: HashMap<String, Attribute>,
    pub generics: HashMap<String, Vec<ResolvableTypes>>,
    pub modifiers: u8,
    pub fields: Vec<Field>,
    pub code: CodeBody,
    pub return_type: Option<ResolvableTypes>,
    pub name: String
}

impl Function {
    pub fn new(attributes: HashMap<String, Attribute>, modifiers: u8, fields: Vec<Field>, generics: HashMap<String, Vec<ResolvableTypes>>,
               code: CodeBody, return_type: Option<ResolvableTypes>, name: String) -> Self {
        return Self {
            attributes,
            generics,
            modifiers,
            fields,
            code,
            return_type,
            name
        };
    }

    pub fn set_generics(&self, replacing: &HashMap<String, ResolvableTypes>) -> Self {
        let mut code = self.code.clone();
        code.set_generics(&replacing);
        let mut return_type = self.return_type.clone();
        if let Some(returning) = &mut return_type {
            returning.set_generic(replacing);
        }
        return Function::new(self.attributes.clone(), self.modifiers,
                             self.fields.iter().map(|field| field.set_generics(&replacing)).collect(),
                             HashMap::new(), code, return_type,
        self.get_mangled_name(replacing));
    }

    pub fn get_mangled_name(&self, replacing: &HashMap<String, ResolvableTypes>) -> String {
        return self.name.clone() + "_" + &display_parenless(&replacing.values().collect(), "_")
    }

    pub fn extract_generics(&self, calling: &Vec<ResolvableTypes>) -> HashMap<String, ResolvableTypes> {
        let mut output = HashMap::new();
        for i in 0..calling.len() {
            if let ResolvableTypes::Resolving(name) = &self.fields.get(i).unwrap().field_type {
                if self.generics.contains_key(name) {
                    let bounds = self.generics.get(name).unwrap();
                    let target = (*calling.get(i).unwrap()).clone();
                    for bound in bounds {
                        if !target.unwrap().is_type(bound.unwrap()) {
                            panic!("Generic {} isn't of type {}!", target, bound);
                        }
                    }
                    output.insert(name.clone(), target);
                }
            }
        }
        return output;
    }

    pub fn finalize(&mut self, type_manager: &mut dyn FinalizedTypeResolver) {
        if self.generics.is_empty() {
            type_manager.finalize_func(self);
        }
    }

    pub fn finalize_code(&mut self, type_manager: &mut dyn FinalizedTypeResolver) {
        type_manager.finalize_code(&self.name);
    }

    pub fn check_args(&self, target: &Vec<&Effects>) -> bool {
        if target.len() != self.fields.len() {
            return false;
        }

        for i in 0..target.len() {
            match target.get(i).unwrap().unwrap().return_type() {
                Some(target) => if target.unwrap() != self.fields.get(i).unwrap().field_type.unwrap() {
                    return false;
                },
                None => return false
            }
        }
        return true;
    }
}

#[derive(Clone, Default)]
pub struct Arguments {
    pub arguments: Vec<Effects>,
}

impl Arguments {
    pub fn new(arguments: Vec<Effects>) -> Self {
        return Self {
            arguments
        };
    }

    pub fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        for arg in &mut self.arguments {
            arg.finalize(type_resolver);
        }
    }
}

#[derive(Clone, Default)]
pub struct CodeBody {
    pub expressions: Vec<Expression>,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>) -> Self {
        return Self {
            expressions
        };
    }

    pub fn is_return(&self) -> bool {
        return self.expressions.iter().find(|expression| expression.is_return()).is_some();
    }
}

impl Effect for CodeBody {
    fn is_return(&self) -> bool {
        for expression in &self.expressions {
            if expression.is_return() {
                return true;
            }
        }
        return false;
    }

    fn has_return(&self) -> bool {
        return false;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        for expression in &mut self.expressions {
            expression.finalize(type_resolver);
        }
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        for expression in &self.expressions {
            if let ExpressionType::Break = expression.expression_type {
                return expression.effect.unwrap().return_type();
            }
        }
        return None;
    }

    fn get_location(&self) -> (u32, u32) {
        todo!()
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        for expression in &mut self.expressions {
            expression.set_generics(replacing);
        }
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
            write!(f, "-> {} ", self.return_type.as_ref().unwrap())?;
        }
        return self.code.format(indent, f);
    }
}

impl DisplayIndented for CodeBody {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\n")?;
        let deeper_indent = indent.to_string() + "    ";
        for expression in &self.expressions {
            expression.format(deeper_indent.as_str(), f)?;
        }
        write!(f, "{}}}", indent)?;
        return Ok(());
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", display(&self.arguments, ", "));
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