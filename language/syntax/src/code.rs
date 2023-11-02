/// This file contains the representation of code in Raven and helper methods to transform that code.
use std::sync::Arc;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use async_recursion::async_recursion;

use crate::{Attribute, SimpleVariableManager, ParsingError, ProcessManager, VariableManager};
use crate::async_util::{AsyncDataGetter, UnparsedType};
use crate::function::{CodeBody, FinalizedCodeBody, CodelessFinalizedFunction};
use crate::r#struct::{BOOL, F64, FinalizedStruct, STR, U64};
use crate::syntax::Syntax;
use crate::top_element_manager::ImplWaiter;
use crate::types::{FinalizedTypes, Types};

/// An expression is a single line of code, containing an effect and the type of expression.
#[derive(Clone, Debug)]
pub struct Expression {
    pub expression_type: ExpressionType,
    pub effect: Effects,
}

/// An expression that has been finalized.
#[derive(Clone, Debug)]
pub struct FinalizedExpression {
    pub expression_type: ExpressionType,
    pub effect: FinalizedEffects,
}

/// the types of expressions: a normal line, a return, or a break (for inside control statements).
#[derive(Clone, Copy, Debug, PartialOrd, PartialEq)]
pub enum ExpressionType {
    Break,
    Return,
    Line,
}

/// A field has a name and a type, see MemberField for the main use of fields.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub field_type: Types,
}

/// A finalized field.
#[derive(Clone, Debug)]
pub struct FinalizedField {
    pub name: String,
    pub field_type: FinalizedTypes,
}

/// A field with modifiers and attributes, for example the arguments of a function or types of a struct.
#[derive(Clone, Debug)]
pub struct MemberField {
    pub modifiers: u8,
    pub attributes: Vec<Attribute>,
    pub field: Field,
}

/// A finalized member field.
#[derive(Clone, Debug)]
pub struct FinalizedMemberField {
    pub modifiers: u8,
    pub attributes: Vec<Attribute>,
    pub field: FinalizedField,
}

impl MemberField {
    pub fn new(modifiers: u8, attributes: Vec<Attribute>, field: Field) -> Self {
        return Self {
            modifiers,
            attributes,
            field,
        };
    }
}

impl Expression {
    pub fn new(expression_type: ExpressionType, effect: Effects) -> Self {
        return Self {
            expression_type,
            effect,
        };
    }
}

impl FinalizedExpression {
    pub fn new(expression_type: ExpressionType, effect: FinalizedEffects) -> Self {
        return Self {
            expression_type,
            effect,
        };
    }
}

impl Field {
    pub fn new(name: String, field_type: Types) -> Self {
        return Self {
            name,
            field_type,
        };
    }
}

/// Effects are single pieces of code which are strung together to make an expression.
/// For example, a single method call, creating a variable, setting a variable, etc... are all effects.
#[derive(Clone, Debug)]
pub enum Effects {
    // A placeholder of no operation, which should be resolved before finalizing.
    NOP(),
    // An effect wrapped in parenthesis, just a wrapper around the effect to prevent issues with operator merging.
    Paren(Box<Effects>),
    // Creates a variable with the given name and value.
    CreateVariable(String, Box<Effects>),
    // Label of jumping to body
    Jump(String),
    // Comparison effect, and label to jump to the first if true, second if false
    CompareJump(Box<Effects>, String, String),
    // A block of code inside the block of code.
    CodeBody(CodeBody),
    // Finds the implementation of the given trait for the given calling type, and calls the given method.
    // Calling, trait to call, function name, args, and return type (if explicitly required)
    ImplementationCall(Box<Effects>, String, String, Vec<Effects>, Option<UnparsedType>),
    // Finds the method with the name and calls it with those arguments.
    // Calling, calling function, function arguments, and return type (if explicitly required, see CodelessFinalizedFunction::degeneric)
    MethodCall(Option<Box<Effects>>, String, Vec<Effects>, Option<UnparsedType>),
    // Sets the variable to a value.
    Set(Box<Effects>, Box<Effects>),
    // Loads variable with the given name.
    LoadVariable(String),
    // Loads a field with the given name from the structure.
    Load(Box<Effects>, String),
    // An unresolved operation, sent to the checker to resolve, with the given arguments.
    Operation(String, Vec<Effects>),
    // Struct to create and a tuple of the name of the field and the argument.
    CreateStruct(UnparsedType, Vec<(String, Effects)>),
    // Creates an array of the given effects.
    CreateArray(Vec<Effects>),
    // Creates a constant of the given type.
    Float(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
    String(String),
}

#[derive(Clone, Debug)]
pub enum FinalizedEffects {
    //  Exclusively used for void returns. Will make the compiler panic.
    NOP(),
    //  Creates a variable.
    CreateVariable(String, Box<FinalizedEffects>, FinalizedTypes),
    // Jumps to the given label.
    Jump(String),
    // Comparison effect, jumps to the given first label if true, or second label if false
    CompareJump(Box<FinalizedEffects>, String, String),
    // Nested code body.
    CodeBody(FinalizedCodeBody),
    // Calls the function on the given value (if any) with the given arguments.
    MethodCall(Option<Box<FinalizedEffects>>, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    // Calls the trait's function with the given arguments.
    GenericMethodCall(Arc<CodelessFinalizedFunction>, FinalizedTypes, Vec<FinalizedEffects>),
    // Sets given reference to given value.
    Set(Box<FinalizedEffects>, Box<FinalizedEffects>),
    // Loads variable with the given name.
    LoadVariable(String),
    // Loads a field reference from the given struct with the given type.
    Load(Box<FinalizedEffects>, String, Arc<FinalizedStruct>),
    // Creates a struct at the given reference, of the given type with a tuple of the index of the argument and the argument.
    CreateStruct(Option<Box<FinalizedEffects>>, FinalizedTypes, Vec<(usize, FinalizedEffects)>),
    // Create an array with the type and values
    CreateArray(Option<FinalizedTypes>, Vec<FinalizedEffects>),
    // Creates the given constant
    Float(f64),
    UInt(u64),
    Bool(bool),
    String(String),
    // Calls a virtual method, usually a downcasted trait, with the given function index, function,
    // and on the given arguments (first argument must be the downcased trait).
    VirtualCall(usize, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    // Downcasts a structure into its trait, which can only be used in a VirtualCall.
    Downcast(Box<FinalizedEffects>, FinalizedTypes),
    // Internally used by low-level verifier to store a type on the heap.
    HeapStore(Box<FinalizedEffects>),
    // Allocates space on the heap.
    HeapAllocate(FinalizedTypes),
    // Loads from the given reference.
    ReferenceLoad(Box<FinalizedEffects>),
    // Stores an effect on the stack.
    StackStore(Box<FinalizedEffects>),
}

impl FinalizedEffects {
    /// Gets the return type of the effect, requiring a variable manager to get
    /// any variables from, or None if the effect has no return type.
    pub fn get_return(&self, variables: &dyn VariableManager) -> Option<FinalizedTypes> {
        let temp = match self {
            FinalizedEffects::NOP() => None,
            FinalizedEffects::Jump(_) => None,
            FinalizedEffects::CompareJump(_, _, _) => None,
            FinalizedEffects::CodeBody(_) => None,
            FinalizedEffects::CreateVariable(_, _, types) => Some(types.clone()),
            FinalizedEffects::MethodCall(_, function, _) =>
                function.return_type.as_ref().map(|inner| {
                    FinalizedTypes::Reference(Box::new(inner.clone()))
                }),
            FinalizedEffects::VirtualCall(_, function, _) =>
                function.return_type.as_ref().map(|inner| {
                    FinalizedTypes::Reference(Box::new(inner.clone()))
                }),
            FinalizedEffects::Set(_, to) => to.get_return(variables),
            FinalizedEffects::LoadVariable(name) => {
                let variable = variables.get_variable(name);
                if let Some(found) = variable {
                    match found {
                        // Generics must be resolved to get a concrete type, so this is a sanity check.
                        FinalizedTypes::Generic(name, _) => {
                            panic!("Unresolved generic {}", name)
                        }
                        FinalizedTypes::GenericType(name, _) => {
                            panic!("Unresolved generic {:?}", name)
                        }
                        _ => return Some(found)
                    }
                }
                // Failed to find a variable with that name.
                panic!("Unresolved variable {} from {:?}", name, variables);
            }
            // Gets the type of the field in the structure with that name.
            FinalizedEffects::Load(_, name, loading) =>
                loading.fields.iter()
                    .find(|field| &field.field.name == name)
                    .map(|field| field.field.field_type.clone()),
            // Returns the structure type.
            FinalizedEffects::CreateStruct(_, types, _) =>
                Some(FinalizedTypes::Reference(Box::new(types.clone()))),
            // Returns the internal constant type.
            FinalizedEffects::Float(_) => Some(FinalizedTypes::Struct(F64.clone())),
            FinalizedEffects::UInt(_) => Some(FinalizedTypes::Struct(U64.clone())),
            FinalizedEffects::Bool(_) => Some(FinalizedTypes::Struct(BOOL.clone())),
            FinalizedEffects::String(_) => Some(FinalizedTypes::Struct(STR.clone())),
            // Stores just return their inner type.
            FinalizedEffects::HeapStore(inner) => inner.get_return(variables),
            FinalizedEffects::StackStore(inner) => inner.get_return(variables),
            // References return their inner type as well.
            FinalizedEffects::ReferenceLoad(inner) => match inner.get_return(variables).unwrap() {
                FinalizedTypes::Reference(inner) => Some(*inner),
                _ => panic!("Tried to load non-reference!")
            },
            // Heap allocations shouldn't get return type checked, even though they have a type.
            FinalizedEffects::HeapAllocate(_) => panic!("Tried to return type a heap allocation!"),
            // Returns the target type as an array type.
            FinalizedEffects::CreateArray(types, _) =>
                types.clone().map(|inner| FinalizedTypes::Array(Box::new(inner))),
            // Downcasts simply return the downcasting target.
            FinalizedEffects::Downcast(_, target) => Some(target.clone()),
            FinalizedEffects::GenericMethodCall(function, _, _) =>
                function.return_type.as_ref().map(|inner| {
                    FinalizedTypes::Reference(Box::new(inner.clone()))
                })
        };
        return temp;
    }

    /// Degenericing replaces every instance of a generic function with its actual type.
    /// This mostly targets FinalizedTypes or function calls and calls the degeneric function on them.
    #[async_recursion]
    pub async fn degeneric(&mut self, process_manager: &Box<dyn ProcessManager>, variables: &mut SimpleVariableManager,
                           syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
        match self {
            // Recursively searches nested effects for method calls.
            FinalizedEffects::NOP() => {}
            FinalizedEffects::CreateVariable(_, first, other) => {
                first.degeneric(process_manager, variables, syntax).await?;
                other.degeneric(process_manager.generics(), syntax, ParsingError::empty(), ParsingError::empty()).await?;
            }
            FinalizedEffects::Jump(_) => {}
            FinalizedEffects::CompareJump(comparing, _, _) =>
                comparing.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::CodeBody(body) => {
                for statement in &mut body.expressions {
                    statement.effect.degeneric(process_manager, variables, syntax).await?;
                }
            }
            FinalizedEffects::MethodCall(calling, method,
                                         effects) => {
                if let Some(inner) = calling {
                    inner.degeneric(process_manager, variables, syntax).await?;
                }
                for effect in &mut *effects {
                    effect.degeneric(process_manager, variables, syntax).await?;
                }
                let manager: Box<dyn ProcessManager> = process_manager.cloned();
                // Calls the degeneric method on the method.
                *method = CodelessFinalizedFunction::degeneric(method.clone(), manager,
                                                               effects, syntax, variables, None).await?;
            }
            FinalizedEffects::GenericMethodCall(function, found_trait, effects) => {
                let mut calling = effects.remove(0);
                calling.degeneric(process_manager, variables, syntax).await?;

                let implementor = calling.get_return(variables).unwrap();
                let implementation = ImplWaiter {
                    syntax: syntax.clone(),
                    return_type: implementor.clone(),
                    data: found_trait.inner_struct().data.clone(),
                    error: ParsingError::empty()
                }.await?;

                let name = function.data.name.split("::").last().unwrap();
                let function = implementation.iter().find(|inner| inner.name.ends_with(&name)).unwrap();

                for effect in &mut *effects {
                    effect.degeneric(process_manager, variables, syntax).await?;
                }
                let mut effects = effects.clone();
                effects.insert(0, calling.clone());
                let function = AsyncDataGetter::new(syntax.clone(), function.clone()).await;
                let function = CodelessFinalizedFunction::degeneric(function.clone(), process_manager.cloned(),
                                                                    &effects, syntax, variables, None).await?;
                *self = FinalizedEffects::MethodCall(None,
                                                     function,
                                                     effects.clone());
            }
            // Virtual calls can't be generic because virtual calls aren't direct calls which can be degenericed.
            FinalizedEffects::VirtualCall(_, _, effects) => {
                for effect in &mut *effects {
                    effect.degeneric(process_manager, variables, syntax).await?;
                }
            }
            FinalizedEffects::Set(setting, value) => {
                setting.degeneric(process_manager, variables, syntax).await?;
                value.degeneric(process_manager, variables, syntax).await?;
            }
            FinalizedEffects::LoadVariable(_) => {}
            FinalizedEffects::Load(effect, _, _) => effect.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::CreateStruct(target, types, effects) => {
                if let Some(found) = target {
                    found.degeneric(process_manager, variables, syntax).await?;
                }
                types.degeneric(process_manager.generics(), syntax,
                                ParsingError::empty(), ParsingError::empty()).await?;
                for (_, effect) in effects {
                    effect.degeneric(process_manager, variables, syntax).await?;
                }
            }
            FinalizedEffects::CreateArray(other, effects) => {
                if let Some(inner) = other {
                    inner.degeneric(process_manager.generics(), syntax, ParsingError::empty(), ParsingError::empty()).await?;
                }
                for effect in effects {
                    effect.degeneric(process_manager, variables, syntax).await?;
                }
            }
            FinalizedEffects::Float(_) => {}
            FinalizedEffects::UInt(_) => {}
            FinalizedEffects::Bool(_) => {}
            FinalizedEffects::String(_) => {}
            FinalizedEffects::HeapStore(storing) =>
                storing.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::HeapAllocate(other) =>
                other.degeneric(process_manager.generics(), syntax,
                                ParsingError::empty(), ParsingError::empty()).await?,
            FinalizedEffects::ReferenceLoad(loading) =>
                loading.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::StackStore(storing) =>
                storing.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::Downcast(_, _) => {}
        }
        return Ok(());
    }
}