use std::sync::Arc;

use data::tokens::Span;

use crate::async_util::UnparsedType;
use crate::program::function::{CodeBody, CodelessFinalizedFunction, FinalizedCodeBody, FunctionData};
use crate::program::r#struct::{BOOL, CHAR, F64, STR, U64};
use crate::program::types::{FinalizedTypes, Types};
use crate::{Attribute, VariableManager};

/// An expression is a single line of code, containing an effect and the type of expression.
#[derive(Clone, Debug)]
pub struct Expression {
    /// The expression type
    pub expression_type: ExpressionType,
    /// The contained code
    pub effect: Effects,
}

/// An expression that has been finalized.
#[derive(Clone, Debug)]
pub struct FinalizedExpression {
    /// The expression type
    pub expression_type: ExpressionType,
    /// The finalized code
    pub effect: FinalizedEffects,
}

/// the types of expressions: a normal line, a return, or a break (for inside control statements).
#[derive(Clone, Debug)]
pub enum ExpressionType {
    /// Breaks break out of a looping control statement like a for or while loop
    Break,
    /// Return returns out of the current function
    Return(Span),
    /// A line does nothing
    Line,
}

/// A field has a name and a type, see MemberField for the main use of fields.
#[derive(Clone, Debug)]
pub struct Field {
    /// The name of the field
    pub name: String,
    /// The field's type
    pub field_type: Types,
}

/// A finalized field.
#[derive(Clone, Debug)]
pub struct FinalizedField {
    /// The name of the field
    pub name: String,
    /// The field's type
    pub field_type: FinalizedTypes,
}

/// A field with modifiers and attributes, for example the arguments of a function or types of a struct.
#[derive(Clone, Debug)]
pub struct MemberField {
    /// The field's modifiers
    pub modifiers: u8,
    /// The field's attributes
    pub attributes: Vec<Attribute>,
    /// The field itself
    pub field: Field,
}

/// A finalized member field.
#[derive(Clone, Debug)]
pub struct FinalizedMemberField {
    /// The field's modifiers
    pub modifiers: u8,
    /// The field's attributes
    pub attributes: Vec<Attribute>,
    /// The field itself
    pub field: FinalizedField,
}

impl PartialEq for ExpressionType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExpressionType::Return(_), ExpressionType::Return(_)) => true,
            (ExpressionType::Line, ExpressionType::Line) => true,
            (ExpressionType::Break, ExpressionType::Break) => true,
            _ => false,
        }
    }
}

impl MemberField {
    /// Creates a new field
    pub fn new(modifiers: u8, attributes: Vec<Attribute>, field: Field) -> Self {
        return Self { modifiers, attributes, field };
    }
}

impl Expression {
    /// Creates a new expression
    pub fn new(expression_type: ExpressionType, effect: Effects) -> Self {
        return Self { expression_type, effect };
    }
}

impl FinalizedExpression {
    /// Creates a new finalized expression
    pub fn new(expression_type: ExpressionType, effect: FinalizedEffects) -> Self {
        return Self { expression_type, effect };
    }
}

impl Field {
    /// Creates a new field
    pub fn new(name: String, field_type: Types) -> Self {
        return Self { name, field_type };
    }
}

/// Effects are single pieces of code which are strung together to make an expression.
/// For example, a single method call, creating a variable, setting a variable, etc... are all effects.
#[derive(Clone, Debug)]
pub struct Effects {
    /// The type of the effect
    pub types: EffectType,
    /// The span of the effect
    pub span: Span,
}

impl Effects {
    /// Creates a new effect
    pub fn new(span: Span, types: EffectType) -> Self {
        return Self { types, span };
    }
}

/// The type of the effect, storing all per-effect data
#[derive(Clone, Debug)]
pub enum EffectType {
    /// A placeholder of no operation, which should be resolved before finalizing.
    NOP,
    /// An effect wrapped in parenthesis, just a wrapper around the effect to prevent issues with operator merging.
    Paren(Box<Effects>),
    /// Creates a variable with the given name and value.
    CreateVariable(String, Box<Effects>),
    /// Label of jumping to body
    Jump(String),
    /// Comparison effect, and label to jump to the first if true, second if false
    CompareJump(Box<Effects>, String, String),
    /// A block of code inside the block of code.
    CodeBody(CodeBody),
    /// Finds the implementation of the given trait for the given calling type, and calls the given method.
    /// Calling, trait to call, function name, and args
    ImplementationCall(Box<Effects>, String, String, Vec<Effects>),
    /// Finds the method with the name and calls it with those arguments.
    /// Calling, calling function, function arguments, and explicit generics
    MethodCall(Option<Box<Effects>>, String, Vec<Effects>, Vec<UnparsedType>),
    /// Sets the variable to a value.
    Set(Box<Effects>, Box<Effects>),
    /// Loads variable with the given name.
    LoadVariable(String),
    /// Loads a field with the given name from the program.
    Load(Box<Effects>, String),
    /// An unresolved operation, sent to the checker to resolve, with the given arguments.
    Operation(String, Vec<Effects>),
    /// Struct to create and a tuple of the name of the field and the argument.
    CreateStruct(UnparsedType, Vec<(String, Effects)>),
    /// Creates an array of the given effects.
    CreateArray(Vec<Effects>),
    /// A float
    Float(f64),
    /// An integer
    Int(i64),
    /// An unsigned integer
    UInt(u64),
    /// A boolean
    Bool(bool),
    /// A character
    Char(char),
    /// A string
    String(String),
}

/// Effects that have been finalized and are ready for compilation
#[derive(Clone, Debug)]
pub struct FinalizedEffects {
    /// The type of the effect
    pub types: FinalizedEffectType,
    /// The span of the effect
    pub span: Span,
}

impl FinalizedEffects {
    /// Creates a new finalized effect
    pub fn new(span: Span, types: FinalizedEffectType) -> Self {
        return Self { types, span };
    }
}

/// Effects that have been finalized and are ready for compilation
#[derive(Clone, Debug)]
pub enum FinalizedEffectType {
    ///  Exclusively used for void returns. Will make the compiler panic.
    NOP,
    ///  Creates a variable.
    CreateVariable(String, Box<FinalizedEffects>, FinalizedTypes),
    /// Jumps to the given label.
    Jump(String),
    /// Comparison effect, jumps to the given first label if true, or second label if false
    CompareJump(Box<FinalizedEffects>, String, String),
    /// Nested code body.
    CodeBody(FinalizedCodeBody),
    /// Calls the function on the given value (if any) with the given arguments and the given return type (if generic). The first arg is the output location
    MethodCall(
        Option<Box<FinalizedEffects>>,
        Arc<CodelessFinalizedFunction>,
        Vec<FinalizedEffects>,
        Vec<(FinalizedTypes, Span)>,
    ),
    /// Calls the trait's function with the given arguments.
    GenericMethodCall(Arc<CodelessFinalizedFunction>, FinalizedTypes, Vec<FinalizedEffects>),
    /// Sets given reference to given value.
    Set(Box<FinalizedEffects>, Box<FinalizedEffects>),
    /// Loads variable with the given name.
    LoadVariable(String),
    /// Loads a field reference from the given struct with the given type.
    Load(Box<FinalizedEffects>, String, FinalizedTypes),
    /// Creates a struct at the given reference, of the given type with a tuple of the index of the argument and the argument.
    CreateStruct(Option<Box<FinalizedEffects>>, FinalizedTypes, Vec<(usize, FinalizedEffects)>),
    /// Creates a float
    Float(f64),
    /// Creates an unsigned int
    UInt(u64),
    /// Creates a boolean
    Bool(bool),
    /// Creates a string
    String(String),
    /// Creates a character
    Char(char),
    /// Calls a virtual method, usually a downcasted trait, with the given function index, function, and generic return type (if any)
    /// and with the given arguments on the given effect (the downcased trait).
    VirtualCall(usize, Arc<CodelessFinalizedFunction>, Box<FinalizedEffects>, Vec<FinalizedEffects>),
    /// Calls a virtual method on a generic type. Same as above, but must degeneric like check_code on EffectType::ImplementationCall
    GenericVirtualCall(usize, Arc<FunctionData>, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    /// Downcasts a program into its trait (with the given functions), which can only be used in a VirtualCall.
    /// The functions are empty until after degenericing
    Downcast(Box<FinalizedEffects>, FinalizedTypes, Vec<Arc<CodelessFinalizedFunction>>),
    /// Internally used by low-level verifier to store a type on the heap.
    HeapStore(Box<FinalizedEffects>),
    /// Allocates space on the heap.
    HeapAllocate(FinalizedTypes),
    /// Loads from the given reference.
    ReferenceLoad(Box<FinalizedEffects>),
    /// Stores an effect on the stack.
    StackStore(Box<FinalizedEffects>),
}

impl FinalizedEffectType {
    /// get_return is async to handle special cases with function return types being generic.
    /// This can only be called on degenericed types and as such can be sync
    pub fn get_nongeneric_return(&self, variables: &dyn VariableManager) -> Option<FinalizedTypes> {
        return match self {
            Self::NOP | Self::Jump(_) | Self::CompareJump(_, _, _) | Self::CodeBody(_) => None,
            // Downcasts simply return the downcasting target.
            Self::CreateVariable(_, _, types) | Self::Downcast(_, types, _) => Some(types.clone()),
            Self::MethodCall(_, function, _, _)
            | Self::GenericMethodCall(function, _, _)
            | Self::VirtualCall(_, function, _, _)
            | Self::GenericVirtualCall(_, _, function, _) => {
                function.return_type.as_ref().map(|inner| FinalizedTypes::Reference(Box::new(inner.clone())))
            }
            Self::LoadVariable(name) => {
                let variable = variables.get_variable(name);
                if variable.is_some() {
                    return variable;
                }
                // Failed to find a variable with that name.
                panic!("Unresolved variable {} from {:?}", name, variables);
            }
            // Gets the type of the field in the program with that name.
            Self::Load(_, name, loading) => loading
                .inner_struct()
                .fields
                .iter()
                .find(|field| &field.field.name == name)
                .map(|field| field.field.field_type.clone()),
            // Returns the program type.
            Self::CreateStruct(_, types, _) => Some(FinalizedTypes::Reference(Box::new(types.clone()))),
            // Returns the internal constant type.
            Self::Float(_) => Some(FinalizedTypes::Struct(F64.clone())),
            Self::UInt(_) => Some(FinalizedTypes::Struct(U64.clone())),
            Self::Bool(_) => Some(FinalizedTypes::Struct(BOOL.clone())),
            Self::String(_) => Some(FinalizedTypes::Struct(STR.clone())),
            Self::Char(_) => Some(FinalizedTypes::Struct(CHAR.clone())),
            // Stores just return their inner type.
            Self::HeapStore(inner) | Self::StackStore(inner) | Self::Set(_, inner) => {
                inner.types.get_nongeneric_return(variables)
            }
            // References return their inner type as well.
            Self::ReferenceLoad(inner) => match inner.types.get_nongeneric_return(variables).unwrap() {
                FinalizedTypes::Reference(inner) => Some(*inner),
                _ => panic!("Tried to load non-reference!"),
            },
            // Heap allocations shouldn't get return type checked, even though they have a type.
            Self::HeapAllocate(_) => panic!("Tried to get a type from a heap alloc!"),
        };
    }
}
