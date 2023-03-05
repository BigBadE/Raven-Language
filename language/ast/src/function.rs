use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::r#struct::TypeMember;
use crate::code::{Effects, Expression, Field};
use crate::{Attribute, DisplayIndented, to_modifiers};
use crate::type_resolver::TypeResolver;
use crate::types::Types;

pub struct Function {
    pub attributes: HashMap<String, Attribute>,
    pub modifiers: u8,
    pub fields: Vec<Field>,
    pub code: CodeBody,
    pub return_type: Option<Rc<Types>>,
    pub name: String,
    //Stored until all structs are loaded
    parsing_fields: Vec<(String, String)>,
    parsing_return: Option<String>,
}

impl Function {
    pub fn new(attributes: HashMap<String, Attribute>, modifiers: u8, fields: Vec<(String, String)>,
               code: CodeBody, return_type: Option<String>, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            fields: Vec::new(),
            code,
            return_type: None,
            name,
            parsing_fields: fields,
            parsing_return: return_type,
        };
    }

    pub fn finalize(&mut self, type_manager: &dyn TypeResolver) {
        for (name, found_type) in &self.parsing_fields {
            match type_manager.get_type(found_type) {
                Some(found) => self.fields.push(Field::new(name.clone(), found)),
                None => panic!("Unknown type {}", found_type)
            }
        }
        self.parsing_fields.clear();

        if let Some(found_type) = &self.parsing_return {
            match type_manager.get_type(found_type) {
                Some(return_type) => self.return_type = Some(return_type),
                None => panic!("Unknown type {}", found_type)
            }
        }
        self.parsing_return = None;
    }

    pub fn check_args(&self, type_resolver: &dyn TypeResolver, target: &Vec<&Effects>) -> bool {
        if target.len() != self.fields.len() {
            return false;
        }

        for i in 0..target.len() {
            match target.get(i).unwrap().unwrap().return_type(type_resolver) {
                Some(target) => if target != self.fields.get(i).unwrap().field_type {
                    return false;
                },
                None => return false
            }
        }
        return true;
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

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
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
    return format!("({})", (&output[..output.len() - 2]).to_string());
}

impl TypeMember for Function {}