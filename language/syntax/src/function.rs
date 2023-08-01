use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc; use no_deadlocks::Mutex;

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
    pub poisoned: Vec<ParsingError>,
}

pub struct UnfinalizedFunction {
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub fields: Vec<ParsingFuture<MemberField>>,
    pub code: CodeBody,
    pub return_type: Option<ParsingFuture<Types>>,
    pub data: Arc<FunctionData>,
}

/// If the code is required to finalize the function, then recursion will deadlock
#[derive(Clone)]
pub struct CodelessFinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>
}

impl Debug for CodelessFinalizedFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{}", self.data.name, debug_parenless(&self.fields, ", "));
    }
}

impl CodelessFinalizedFunction {
    pub fn add_code(self, code: FinalizedCodeBody) -> FinalizedFunction {
        return FinalizedFunction {
            generics: self.generics,
            fields: self.fields,
            code,
            return_type: self.return_type,
            data: self.data,
        };
    }
}

#[derive(Clone, Debug)]
pub struct FinalizedFunction {
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub fields: Vec<FinalizedMemberField>,
    pub code: FinalizedCodeBody,
    pub return_type: Option<FinalizedTypes>,
    pub data: Arc<FunctionData>,
}

impl FinalizedFunction {
    pub fn to_codeless(&self) -> CodelessFinalizedFunction {
        return CodelessFinalizedFunction {
            generics: self.generics.clone(),
            fields: self.fields.clone(),
            return_type: self.return_type.clone(),
            data: self.data.clone(),
        }
    }
}

impl FunctionData {
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String) -> Self {
        return Self {
            attributes,
            modifiers,
            name,
            poisoned: Vec::new(),
        };
    }

    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self {
            attributes: Vec::new(),
            modifiers: 0,
            name,
            poisoned: vec!(error),
        };
    }
}

#[async_trait]
impl TopElement for FunctionData {
    type Unfinalized = UnfinalizedFunction;
    type Finalized = CodelessFinalizedFunction;

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

    async fn verify(current: UnfinalizedFunction, syntax: Arc<Mutex<Syntax>>, resolver: Box<dyn NameResolver>, process_manager: Box<dyn ProcessManager>) {
        let name = current.data.name.clone();
        let output = process_manager.verify_func(current, resolver, &syntax).await;
        //SAFETY: compiling is only accessed from here and in the compiler, and neither is dropped
        //until after both finish.
        unsafe {
            Arc::get_mut_unchecked(&mut syntax.lock().unwrap().compiling)
        }.insert(name, Arc::new(output));
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

#[derive(Clone, Default, Debug)]
pub struct FinalizedCodeBody {
    pub label: String,
    pub expressions: Vec<FinalizedExpression>,
    pub returns: bool
}

impl CodeBody {
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self {
            label,
            expressions,
        };
    }
}

impl FinalizedCodeBody {
    pub fn new(expressions: Vec<FinalizedExpression>, label: String, returns: bool) -> Self {
        return Self {
            label,
            expressions,
            returns
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
    return output[..output.len() - 1].to_string();
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

pub fn debug_parenless<T>(input: &Vec<T>, deliminator: &str) -> String where T: Debug {
    if input.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for element in input {
        output += &*format!("{:?}{}", element, deliminator);
    }

    return (&output[..output.len() - deliminator.len()]).to_string();
}

impl Hash for FunctionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for FunctionData {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name;
    }
}

impl Eq for FunctionData {

}