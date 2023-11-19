#![feature(box_into_inner)]
#![feature(get_mut_unchecked)]
#![feature(fn_traits)]

use crate::async_util::{HandleWrapper, NameResolver};
use crate::function::{
    CodeBody, CodelessFinalizedFunction, FinalizedFunction, FunctionData, UnfinalizedFunction,
};
use crate::r#struct::{FinalizedStruct, StructData, UnfinalizedStruct};
use crate::syntax::Syntax;
use crate::top_element_manager::TopElementManager;
use crate::types::{FinalizedTypes, Types};
use async_trait::async_trait;
use chalk_solve::rust_ir::ImplDatum;
use indexmap::IndexMap;
/// A file containing various structures used throughout the language:
/// - Modifiers: modifiers on structures, traits, and functions. Like public, internal, etc...
///     - Modifier helper functions for compressing to/from and checking modifier lists in u8 form
/// - Attributes: Data attached to objects like functions or structs in #[attribute(value)] form.
///     - Attribute helper functions for checking if attributes exist and getting values
/// - Process Manager trait used for passing parsed data to later compilation steps
/// - Variable Manager and a simple implementation for keeping track of the variables when parsing a function
/// - Data Type trait used a simple wrapper to access the static data (see FunctionData or StructData) of an object with data
/// - Top Element trait used to allow generic access to function and struct types
/// - Trait implementors struct for storing implementor data
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

pub mod async_util;
pub mod chalk_interner;
pub mod chalk_support;
pub mod code;
pub mod function;
pub mod operation_util;
pub mod r#struct;
pub mod syntax;
pub mod top_element_manager;
pub mod types;

//Re-export ParsingError
use crate::chalk_interner::ChalkIr;
pub use data::ParsingError;

// An alias for parsing types, which must be pinned and boxed because Rust generates different impl Futures
// for different functions, so they must be box'd into one type to be passed correctly to ParsingTypes.
pub type ParsingFuture<T> = Pin<Box<dyn Future<Output = Result<T, ParsingError>> + Send + Sync>>;

// All the modifiers, used for modifier parsing and debug output.
pub static MODIFIERS: [Modifier; 5] = [
    Modifier::Public,
    Modifier::Protected,
    Modifier::Extern,
    Modifier::Internal,
    Modifier::Operation,
];

// All the modifiers structures/functions/fields can have
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Modifier {
    Public = 0b1,
    Protected = 0b10,
    Extern = 0b100,
    Internal = 0b1000,
    Operation = 0b1_0000,
    // Hidden from the user, only used internally
    Trait = 0b10_0000,
}

impl Display for Modifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            Modifier::Public => write!(f, "pub"),
            Modifier::Protected => write!(f, "pub(proj)"),
            Modifier::Extern => write!(f, "extern"),
            Modifier::Internal => write!(f, "internal"),
            Modifier::Operation => write!(f, "operation"),
            Modifier::Trait => panic!("Shouldn't display trait modifier!"),
        };
    }
}

/// Gets the modifier in numerical form from list form
pub fn get_modifier(modifiers: &[Modifier]) -> u8 {
    let mut sum = 0;
    for modifier in modifiers {
        sum += *modifier as u8;
    }

    return sum;
}

/// Checks if the numerical modifier contains the given modifier
pub fn is_modifier(modifiers: u8, target: Modifier) -> bool {
    let target = target as u8;
    return modifiers & target == target as u8;
}

/// Converts the numerical form of modifiers to list form
pub fn to_modifiers(from: u8) -> Vec<Modifier> {
    let mut modifiers = Vec::default();
    for modifier in MODIFIERS {
        if from & (modifier as u8) != 0 {
            modifiers.push(modifier)
        }
    }

    return modifiers;
}

// A simple attribute over structures or functions, potentially used later in the process
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Attribute {
    Basic(String),
    Integer(String, i64),
    Bool(String, bool),
    String(String, String),
}

impl Attribute {
    /// Finds the attribute given the name
    pub fn find_attribute<'a>(name: &str, attributes: &'a Vec<Attribute>) -> Option<&'a Attribute> {
        for attribute in attributes {
            if match attribute {
                Attribute::Basic(found) => found == name,
                Attribute::Integer(found, _) => found == name,
                Attribute::Bool(found, _) => found == name,
                Attribute::String(found, _) => found == name,
            } {
                return Some(attribute);
            }
        }
        return None;
    }

    pub fn as_string_attribute(&self) -> Option<&String> {
        match self {
            Attribute::String(_, value) => Some(value),
            _ => None,
        }
    }

    pub fn as_int_attribute(&self) -> Option<i64> {
        match self {
            Attribute::Integer(_, value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bool_attribute(&self) -> Option<bool> {
        match self {
            Attribute::Bool(_, value) => Some(*value),
            _ => None,
        }
    }
}

#[async_trait]
pub trait ProcessManager: Send + Sync {
    fn handle(&self) -> &Arc<Mutex<HandleWrapper>>;

    async fn verify_func(
        &self,
        function: UnfinalizedFunction,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> (CodelessFinalizedFunction, CodeBody);

    async fn verify_code(
        &self,
        function: CodelessFinalizedFunction,
        code: CodeBody,
        resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedFunction;

    async fn verify_struct(
        &self,
        structure: UnfinalizedStruct,
        resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedStruct;

    fn generics(&self) -> &HashMap<String, FinalizedTypes>;

    fn mut_generics(&mut self) -> &mut HashMap<String, FinalizedTypes>;

    fn cloned(&self) -> Box<dyn ProcessManager>;
}

#[derive(Debug, Clone)]
pub struct SimpleVariableManager {
    pub variables: HashMap<String, FinalizedTypes>,
}

impl SimpleVariableManager {
    pub fn for_function(codeless: &CodelessFinalizedFunction) -> Self {
        let mut variable_manager = SimpleVariableManager {
            variables: HashMap::default(),
        };

        for field in &codeless.arguments {
            variable_manager
                .variables
                .insert(field.field.name.clone(), field.field.field_type.clone());
        }

        return variable_manager;
    }
}

impl VariableManager for SimpleVariableManager {
    fn get_variable(&self, name: &String) -> Option<FinalizedTypes> {
        return self.variables.get(name).cloned();
    }
}

// A variable manager used for getting return types from effects
pub trait VariableManager: Debug {
    fn get_variable(&self, name: &String) -> Option<FinalizedTypes>;
}

pub trait DataType<T: TopElement> {
    // The element's data
    fn data(&self) -> &Arc<T>;
}

// Top elements are structures or functions
#[async_trait]
pub trait TopElement
where
    Self: Sized,
{
    type Unfinalized: DataType<Self>;
    type Finalized;

    // Element id
    fn set_id(&mut self, id: u64);

    // Poisons the element, adding an error to it and forcing users to ignore issues with it
    fn poison(&mut self, error: ParsingError);

    // Whether the top element is a function and has the operator modifier
    fn is_operator(&self) -> bool;

    // Whether the top element is a trait or trait member
    fn is_trait(&self) -> bool;

    // All errors on the element
    fn errors(&self) -> &Vec<ParsingError>;

    // Name of the element
    fn name(&self) -> &String;

    // Creates a new poisoned structure of the element
    fn new_poisoned(name: String, error: ParsingError) -> Self;

    // Verifies the top element: de-genericing, checking effect arguments, lifetimes, etc...
    async fn verify(
        handle: Arc<Mutex<HandleWrapper>>,
        current: Self::Unfinalized,
        syntax: Arc<Mutex<Syntax>>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    );

    // Gets the getter for that type on the syntax
    fn get_manager(syntax: &mut Syntax) -> &mut TopElementManager<Self>;
}

// An impl block for a type
pub struct TraitImplementor {
    pub base: ParsingFuture<Types>,
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    pub implementor: ParsingFuture<Types>,
    pub attributes: Vec<Attribute>,
    pub functions: Vec<UnfinalizedFunction>,
}

// Finished impl block for a type.
// Ex: impl<T> Iter<T> for NumberIter<T>
#[derive(Clone)]
pub struct FinishedTraitImplementor {
    //Would be Iter<T>
    pub target: FinalizedTypes,
    //Would be NumberIter<T>
    pub base: FinalizedTypes,
    pub chalk_type: Arc<ImplDatum<ChalkIr>>,
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    pub attributes: Vec<Attribute>,
    pub functions: Vec<Arc<FunctionData>>,
}
