use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::{Attribute, DisplayIndented, ParsingError, TopElement, to_modifiers, Types, ProcessManager, Syntax, AsyncGetter, is_modifier, Modifier, ParsingFuture};
use crate::async_util::NameResolver;
use crate::code::{Expression, MemberField};
use crate::syntax::ParsingType;

#[derive(Clone)]
pub struct Function {
    pub attributes: Vec<Attribute>,
    pub generics: IndexMap<String, Vec<ParsingType<Types>>>,
    pub modifiers: u8,
    pub fields: Vec<ParsingType<MemberField>>,
    pub code: ParsingType<CodeBody>,
    pub return_type: Option<ParsingType<Types>>,
    pub name: String,
    pub poisoned: Vec<ParsingError>
}

impl Function {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8,
               fields: Vec<ParsingType<MemberField>>, generics: IndexMap<String, Vec<ParsingType<Types>>>,
               code: ParsingFuture<CodeBody>, return_type: Option<ParsingType<Types>>, name: String) -> Self {
        return Self {
            attributes,
            generics,
            modifiers,
            fields,
            code: ParsingType::new(code),
            return_type,
            name,
            poisoned: Vec::new()
        };
    }
    
    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            generics: IndexMap::new(),
            modifiers: 0,
            fields: Vec::new(),
            code: ParsingType::new_done(CodeBody::new(Vec::new(), "poison".to_string())),
            return_type: None,
            name,
            poisoned: vec!(error)
        }
    }
}

#[async_trait]
impl TopElement for Function {
    fn poison(&mut self, error: ParsingError) {
        self.poisoned.push(error);
    }

    fn is_operator(&self) -> bool {
        return is_modifier(self.modifiers, Modifier::Operation);
    }

    fn errors(&self) -> &Vec<ParsingError> {
        return &self.poisoned;
    }

    fn name(&self) -> &String {
        return &self.name;
    }

    fn new_poisoned(name: String, error: ParsingError) -> Self {
        return Function::poisoned(name, error);
    }

    async fn verify(mut current: Arc<Self>, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        process_manager.verify_func(unsafe { Arc::get_mut_unchecked(&mut current) }, resolver, syntax).await;
    }

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self> {
        return &mut syntax.functions;
    }
}

#[derive(Clone, Default, Debug)]
pub struct CodeBody {
    pub label: String,
    pub expressions: Vec<Expression>,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self {
            label,
            expressions
        };
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
            for (_name, _generic) in &self.generics {
                write!(f, "TODO")?;
            }
            write!(f, ">")?;
        }

        write!(f, "{} ", display(&self.fields.iter().map(|field| field.assume_finished()).collect::<Vec<_>>(), ", "))?;

        if self.return_type.is_some() {
            write!(f, "-> {} ", self.return_type.as_ref().unwrap().assume_finished())?;
        }
        // self.code.format(indent, f)
        return write!(f, " TODO");
    }
}

impl DisplayIndented for CodeBody {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {{\n", self.label)?;
        let deeper_indent = indent.to_string() + "    ";
        for expression in &self.expressions {
            expression.format(deeper_indent.as_str(), f)?;
        }
        write!(f, "{}}}", indent)?;
        return Ok(());
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

pub fn display_indented<T>(f: &mut Formatter<'_>, input: &Vec<T>, space: &str, deliminator: &str)
    -> std::fmt::Result where T: DisplayIndented {
    write!(f, "(")?;
    for element in input {
        element.format(space, f)?;
        write!(f, "{}", deliminator)?;
    }

    return write!(f, ")");
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