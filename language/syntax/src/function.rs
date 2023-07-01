use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::{Attribute, DisplayIndented, ParsingError, TopElement, Types, ProcessManager, Syntax,
            AsyncGetter, is_modifier, Modifier, ParsingFuture};
use crate::async_util::NameResolver;
use crate::code::{Expression, FinalizedExpression, FinalizedMemberField, MemberField};
use crate::types::FinalizedTypes;

#[derive(Clone, Debug)]
pub struct FunctionData {
    pub attributes: Vec<Attribute>,
    pub modifiers: u8,
    pub name: String,
    pub poisoned: Vec<ParsingError>
}

pub struct UnfinalizedFunction {
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub fields: Vec<ParsingFuture<MemberField>>,
    pub code: ParsingFuture<CodeBody>,
    pub return_type: Option<ParsingFuture<Types>>,
    pub data: Arc<FunctionData>
}

#[derive(Clone, Debug)]
pub struct FinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub code: FinalizedCodeBody,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>
}

impl FunctionData {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            name,
            poisoned: Vec::new()
        };
    }
    
    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            modifiers: 0,
            name,
            poisoned: vec!(error)
        }
    }
}

#[async_trait]
impl TopElement<FinalizedFunction> for FunctionData {
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
        return FunctionData::poisoned(name, error);
    }

    async fn verify(mut current: Arc<Self>, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        unsafe {
            process_manager.verify_func(Arc::get_mut_unchecked(&mut current), resolver, &syntax).await;
            Arc::get_mut_unchecked(&mut syntax.lock().unwrap().compiling).insert(current.name.clone(), current);
        }
    }

    fn get_manager(syntax: &mut Syntax) -> &mut AsyncGetter<Self, FinalizedFunction> {
        return &mut syntax.functions;
    }
}

#[derive(Clone, Default, Debug)]
pub struct CodeBody {
    pub label: String,
    pub expressions: Vec<Expression>,
}

#[derive(Clone, Default, Debug)]
pub struct FinalizedCodeBody {
    pub label: String,
    pub expressions: Vec<FinalizedExpression>,
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self {
            label,
            expressions
        };
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