#![feature(box_into_inner)]
#![feature(get_mut_unchecked)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]
#![feature(async_fn_traits)]

use crate::async_util::{HandleWrapper, NameResolver};
use crate::program::function::{CodeBody, CodelessFinalizedFunction, FinalizedFunction, FunctionData, UnfinalizedFunction};
use crate::program::r#struct::{FinalizedStruct, StructData, UnfinalizedStruct};
use crate::program::syntax::Syntax;
use crate::program::types::{FinalizedTypes, Types};
use crate::top_element_manager::TopElementManager;
use async_trait::async_trait;
use chalk_solve::rust_ir::ImplDatum;
use indexmap::IndexMap;
use parking_lot::Mutex;
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
use std::hash::Hash;
use std::pin::Pin;
use std::sync::Arc;

/// Utility async functions for things like getting types
pub mod async_util;
/// The interner required to use chalk
pub mod chalk_interner;
/// Implements the chalk types for Syntax
pub mod chalk_support;
/// Has all the error-related structs
pub mod errors;
/// Utility functions for operations
pub mod operation_util;
/// Handles the types required to hold the program in memory
pub mod program;
/// Top element manager is a utility type used to manage top elements like funcs or structs
pub mod top_element_manager;

//Re-export ParsingError
use crate::chalk_interner::ChalkIr;
use crate::errors::ParsingError;
use data::tokens::Span;

/// An alias for parsing types, which must be pinned and boxed because Rust generates different impl Futures
/// for different functions, so they must be box'd into one type to be passed correctly to ParsingTypes.
pub type ParsingFuture<T> = Pin<Box<dyn Future<Output = Result<T, ParsingError>> + Send>>;

/// All the modifiers, used for modifier parsing and debug output.
pub static MODIFIERS: [Modifier; 4] = [Modifier::Public, Modifier::Protected, Modifier::Extern, Modifier::Internal];

/// All the modifiers structures/functions/fields can have
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Modifier {
    /// Public can be accessed from anywhere
    Public = 0b1,
    /// Protected can only be accessed from the same project
    Protected = 0b10,
    /// Extern is linked to an external binary
    Extern = 0b100,
    /// Internal is implemented by the compiler
    Internal = 0b1000,
    /// Hidden from the user, only used internally
    Trait = 0b1_0000,
}

impl Display for Modifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            Modifier::Public => write!(f, "pub"),
            Modifier::Protected => write!(f, "pub(proj)"),
            Modifier::Extern => write!(f, "extern"),
            Modifier::Internal => write!(f, "internal"),
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

/// A simple attribute over structures or functions, potentially used later in the process
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Attribute {
    /// #[my_attribute]
    Basic(String),
    /// #[my_attribute(2)]
    Integer(String, i64),
    /// #[my_attribute(false)]
    Bool(String, bool),
    /// #[my_attribute(Some Text)]
    String(String, String),
}

/// An attribute can be added to a struct/func to pass extra data to the compiler
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

    /// Converts the attribute to a string attribute or returns None if it's a different type
    pub fn as_string_attribute(&self) -> Option<&String> {
        match self {
            Attribute::String(_, value) => Some(value),
            _ => None,
        }
    }

    /// Converts the attribute to an int attribute or returns None if it's a different type
    pub fn as_int_attribute(&self) -> Option<i64> {
        match self {
            Attribute::Integer(_, value) => Some(*value),
            _ => None,
        }
    }

    /// Converts the attribute to a bool attribute or returns None if it's a different type
    pub fn as_bool_attribute(&self) -> Option<bool> {
        match self {
            Attribute::Bool(_, value) => Some(*value),
            _ => None,
        }
    }
}

/// The ProcessManager is used to send data to later steps of compilation
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// The handle can be used to spawn async tasks
    fn handle(&self) -> &Arc<Mutex<HandleWrapper>>;

    /// Verifies a function, returning its codeless verified form and the code
    async fn verify_func(
        &self,
        function: UnfinalizedFunction,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> (CodelessFinalizedFunction, CodeBody);

    /// Verifies the code of a function, returning the finalized type
    async fn verify_code(
        &self,
        function: CodelessFinalizedFunction,
        code: CodeBody,
        resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedFunction;

    /// Degenerics the code of a function
    async fn degeneric_code(&self, function: Arc<CodelessFinalizedFunction>, syntax: &Arc<Mutex<Syntax>>);

    /// Verifies a struct, returning the finalized type
    async fn verify_struct(
        &self,
        structure: UnfinalizedStruct,
        resolver: Box<dyn NameResolver>,
        syntax: &Arc<Mutex<Syntax>>,
    ) -> FinalizedStruct;

    /// Gets the current function generics
    fn generics(&self) -> &HashMap<String, FinalizedTypes>;

    /// Gets the current function generics mutably
    fn mut_generics(&mut self) -> &mut HashMap<String, FinalizedTypes>;

    /// Clones the process manager, generally pretty fast because most data is Arc'd
    fn cloned(&self) -> Box<dyn ProcessManager>;
}

/// A simple manager for variables in a function
#[derive(Debug, Clone)]
pub struct SimpleVariableManager {
    /// The variables and their type
    pub variables: HashMap<String, FinalizedTypes>,
}

impl SimpleVariableManager {
    /// Gets the variable manager for the function, filling in the function parameters
    pub fn for_function(codeless: &CodelessFinalizedFunction) -> Self {
        let mut variable_manager = SimpleVariableManager { variables: HashMap::default() };

        for field in &codeless.arguments {
            variable_manager.variables.insert(field.field.name.clone(), field.field.field_type.clone());
        }

        return variable_manager;
    }

    /// Gets the variable manager for the function, filling in the function parameters
    pub fn for_final_function(codeless: &FinalizedFunction) -> Self {
        let mut variable_manager = SimpleVariableManager { variables: HashMap::default() };

        for field in &codeless.fields {
            variable_manager.variables.insert(field.field.name.clone(), field.field.field_type.clone());
        }

        return variable_manager;
    }
}

impl VariableManager for SimpleVariableManager {
    fn get_variable(&self, name: &String) -> Option<FinalizedTypes> {
        return self.variables.get(name).cloned();
    }
}

/// A variable manager used for getting return types from effects
pub trait VariableManager: Debug {
    fn get_variable(&self, name: &String) -> Option<FinalizedTypes>;
}

/// Something that has an inner immutable data type (either FunctionData or StructData)
pub trait DataType<T: TopElement> {
    /// The element's data
    fn data(&self) -> &Arc<T>;
}

/// Top elements are structures or functions
#[async_trait]
pub trait TopElement
where
    Self: Sized + Clone + PartialEq + Eq + Hash,
{
    /// The unfinalized type of this top element
    type Unfinalized: DataType<Self>;
    /// The finalized type of this top element
    type Finalized;

    /// Span
    fn get_span(&self) -> &Span;

    /// Whether the top element is a function and has the operator modifier
    fn is_operator(&self) -> bool;

    /// Whether the top element is a trait or trait member
    fn is_trait(&self) -> bool;

    /// Returns a default self type
    fn default(&self, id: u64) -> Arc<Self>;

    /// Returns the id
    fn id(&self) -> Option<u64>;

    /// All errors on the element
    fn errors(&self) -> &Vec<ParsingError>;

    /// Name of the element
    fn name(&self) -> &String;

    /// Creates a new poisoned program of the element
    fn new_poisoned(name: String, error: ParsingError) -> Self;

    /// Verifies the top element: de-genericing, checking effect arguments, lifetimes, etc...
    async fn verify(
        handle: Arc<Mutex<HandleWrapper>>,
        current: Self::Unfinalized,
        syntax: Arc<Mutex<Syntax>>,
        resolver: Box<dyn NameResolver>,
        process_manager: Box<dyn ProcessManager>,
    ) -> Result<(), ParsingError>;

    /// Gets the getter for that type on the syntax
    fn get_manager(syntax: &mut Syntax) -> &mut TopElementManager<Self>;
}

/// An impl block for a type
pub struct TraitImplementor {
    /// Base type
    pub base: ParsingFuture<Types>,
    /// Type being implemented
    pub implementor: Option<ParsingFuture<Types>>,
    /// The implementor's generics
    pub generics: IndexMap<String, Vec<ParsingFuture<Types>>>,
    /// The implementor's attributes
    pub attributes: Vec<Attribute>,
    /// The implementor's functions
    pub functions: Vec<UnfinalizedFunction>,
}

/// Finished impl block for a type.
/// Ex: impl<T> Iter<T> for NumberIter<T>
#[derive(Clone)]
pub struct FinishedTraitImplementor {
    /// Would be Iter<T>
    pub target: FinalizedTypes,
    /// Would be NumberIter<T>
    pub base: FinalizedTypes,
    /// The implementation as a chalk type
    pub chalk_type: Arc<ImplDatum<ChalkIr>>,
    /// The generics declared by this implementor
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    /// The attributes on this implementor
    pub attributes: Vec<Attribute>,
    /// All ths functions in this implementor
    pub functions: Vec<Arc<FunctionData>>,
}

/// Finished impl block for a type.
/// Ex: impl<T> Iter<T>
#[derive(Clone)]
pub struct FinishedStructImplementor {
    /// Would be Iter<T>
    pub target: FinalizedTypes,
    /// The generics declared by this implementor
    pub generics: IndexMap<String, Vec<FinalizedTypes>>,
    /// The attributes on this implementor
    pub attributes: Vec<Attribute>,
    /// All ths functions in this implementor
    pub functions: Vec<Arc<FunctionData>>,
}
