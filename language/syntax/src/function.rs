use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::Mutex;

use indexmap::IndexMap;

use async_trait::async_trait;
use data::tokens::Span;

use crate::async_util::{HandleWrapper, NameResolver};
use crate::code::{Expression, FinalizedExpression, FinalizedMemberField, MemberField};
use crate::types::FinalizedTypes;
use crate::{
    is_modifier, Attribute, DataType, Modifier, ParsingError, ParsingFuture, ProcessManager, Syntax, TopElement,
    TopElementManager, Types,
};

/// The static data of a function, which is set during parsing and immutable throughout the entire compilation process.
/// Generics will copy this and change the name and types, but never modify the original.
#[derive(Clone, Debug)]
pub struct FunctionData {
    /// The function's attributes
    pub attributes: Vec<Attribute>,
    /// The function's modifiers
    pub modifiers: u8,
    /// The function's name
    pub name: String,
    /// The function's span
    pub span: Span,
    /// The function's errors if it has been poison'd
    pub poisoned: Vec<ParsingError>,
}

impl FunctionData {
    /// Creates a new function
    pub fn new(attributes: Vec<Attribute>, modifiers: u8, name: String, span: Span) -> Self {
        return Self { attributes, modifiers, name, span, poisoned: Vec::default() };
    }

    /// Creates an empty function data that errored while parsing.
    pub fn poisoned(name: String, error: ParsingError) -> Self {
        return Self { attributes: Vec::default(), modifiers: 0, name, span: error.span.clone(), poisoned: vec![error] };
    }
}

/// Allows generic access to FunctionData.
#[async_trait]
impl TopElement for FunctionData {
    type Unfinalized = UnfinalizedFunction;
    type Finalized = CodelessFinalizedFunction;

    fn get_span(&self) -> &Span {
        return &self.span;
    }

    fn set_id(&mut self, _id: u64) {
        //Ignored. Funcs don't have IDs
    }

    fn is_operator(&self) -> bool {
        return false;
    }

    fn is_trait(&self) -> bool {
        return is_modifier(self.modifiers, Modifier::Trait);
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

    /// Verifies the function and adds it to the compiler after it finished verifying.
    async fn verify(
        handle: Arc<Mutex<HandleWrapper>>,
        current: UnfinalizedFunction,
        syntax: Arc<Mutex<Syntax>>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) {
        let name = current.data.name.clone();
        // Get the codeless finalized function and the code from the function.
        let (codeless_function, code) = process_manager.verify_func(current, &syntax).await;

        // Finalize the code and combine it with the codeless finalized function.
        let finalized_function = process_manager.verify_code(codeless_function.clone(), code, resolver, &syntax).await;
        let finalized_function = Arc::new(finalized_function);

        // Add the finalized code to the compiling list.
        Syntax::add_compiling(process_manager, finalized_function.clone(), &syntax).await;
        handle.lock().unwrap().finish_task(&name);
    }

    fn get_manager(syntax: &mut Syntax) -> &mut TopElementManager<Self> {
        return &mut syntax.functions;
    }
}

/// An unfinalized function is the unlinked function directly after parsing, with no code.
/// Code is finalizied separately and combined with this to make a FinalizedFunction.
pub struct UnfinalizedFunction {
    /// The ordered generics of the function
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    /// The function's fields
    pub fields: Vec<ParsingFuture<MemberField>>,
    /// The function's code
    pub code: CodeBody,
    /// The function's return type
    pub return_type: Option<ParsingFuture<Types>>,
    /// The function's data
    pub data: Arc<FunctionData>,
    /// The function's parent
    pub parent: Option<ParsingFuture<Types>>,
}

/// Gives generic access to the function data.
impl DataType<FunctionData> for UnfinalizedFunction {
    fn data(&self) -> &Arc<FunctionData> {
        return &self.data;
    }
}

/// If the code is required to finalize the function, then recursive function calls will deadlock.
/// That's why this codeless variant exists, which allows the function data to be finalized before the code itself.
/// This is combined with the FinalizedCodeBody into a FinalizedFunction which is passed to the compiler.
/// (see add_code below)
#[derive(Clone, Debug)]
pub struct CodelessFinalizedFunction {
    /// The function's generics
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    /// The function's arguments
    pub arguments: Vec<FinalizedMemberField>,
    /// The function's return type
    pub return_type: Option<FinalizedTypes>,
    /// The function's data
    pub data: Arc<FunctionData>,
    /// The parent structure
    pub parent: Option<FinalizedTypes>,
}

impl CodelessFinalizedFunction {
    /// Combines the CodelessFinalizedFunction with a FinalizedCodeBody to get a FinalizedFunction.
    pub fn add_code(self, code: FinalizedCodeBody) -> FinalizedFunction {
        return FinalizedFunction {
            generics: self.generics,
            fields: self.arguments,
            code,
            return_type: self.return_type,
            data: self.data,
        };
    }
}

/// A finalized function, which is ready to be compiled and has been checked of any errors.
#[derive(Clone, Debug)]
pub struct FinalizedFunction {
    /// The function's generics
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    /// The function's fields
    pub fields: Vec<FinalizedMemberField>,
    /// The function's code
    pub code: FinalizedCodeBody,
    /// The function's return type
    pub return_type: Option<FinalizedTypes>,
    /// The function's data
    pub data: Arc<FunctionData>,
}

impl FinalizedFunction {
    /// Recreates the CodelessFinalizedFunction
    pub fn to_codeless(&self) -> CodelessFinalizedFunction {
        return CodelessFinalizedFunction {
            generics: self.generics.clone(),
            arguments: self.fields.clone(),
            return_type: self.return_type.clone(),
            data: self.data.clone(),
            parent: None,
        };
    }
}

/// A body of code, each body must have a label for jump effects to jump to.
/// ! Each nested CodeBody MUST have a jump or return or else the compiler will error !
#[derive(Clone, Default, Debug)]
pub struct CodeBody {
    /// A unique label for this code body, never shown to the user but used by the compiler for jumps
    pub label: String,
    /// The code in this code body
    pub expressions: Vec<Expression>,
}

/// A finalized body of code.
#[derive(Clone, Default, Debug)]
pub struct FinalizedCodeBody {
    /// A unique label for this code body, never shown to the user but used by the compiler for jumps
    pub label: String,
    /// The code in this code body
    pub expressions: Vec<FinalizedExpression>,
    /// Whether every code path in this code body returns
    pub returns: bool,
}

impl CodeBody {
    /// Creates a new code body
    pub fn new(expressions: Vec<Expression>, label: String) -> Self {
        return Self { label, expressions };
    }
}

impl FinalizedCodeBody {
    /// Creates a new code body
    pub fn new(expressions: Vec<FinalizedExpression>, label: String, returns: bool) -> Self {
        return Self { label, expressions, returns };
    }
}

/// Helper functions to display types.
pub fn display<T>(input: &Vec<T>, deliminator: &str) -> String
where
    T: Display,
{
    if input.is_empty() {
        return "()".to_string();
    }

    let mut output = String::default();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
    }

    return format!("({})", (&output[..output.len() - deliminator.len()]).to_string());
}

/// Helper functions to display types without parenthesis.
pub fn display_parenless<T>(input: &Vec<T>, deliminator: &str) -> String
where
    T: Display,
{
    if input.is_empty() {
        return String::default();
    }

    let mut output = String::default();
    for element in input {
        output += &*format!("{}{}", element, deliminator);
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

impl Eq for FunctionData {}
