use std::fmt::{Display, Formatter};
use std::sync::Arc;
#[cfg(debug_assertions)]
use no_deadlocks::Mutex;
#[cfg(not(debug_assertions))]
use std::sync::Mutex;
use async_recursion::async_recursion;

use crate::{Attribute, SimpleVariableManager, ParsingError, ProcessManager, VariableManager};
use crate::async_util::UnparsedType;
use crate::function::{CodeBody, FinalizedCodeBody, CodelessFinalizedFunction};
use crate::r#struct::{BOOL, F64, FinalizedStruct, STR, U64};
use crate::syntax::Syntax;
use crate::types::{FinalizedTypes, Types};

#[derive(Clone, Debug)]
pub struct Expression {
    pub expression_type: ExpressionType,
    pub effect: Effects,
}

#[derive(Clone, Debug)]
pub struct FinalizedExpression {
    pub expression_type: ExpressionType,
    pub effect: FinalizedEffects,
}

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq)]
pub enum ExpressionType {
    Break,
    Return,
    Line,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub field_type: Types,
}

#[derive(Clone, Debug)]
pub struct FinalizedField {
    pub name: String,
    pub field_type: FinalizedTypes,
}

#[derive(Clone, Debug)]
pub struct MemberField {
    pub modifiers: u8,
    pub attributes: Vec<Attribute>,
    pub field: Field,
}

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

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

#[derive(Clone, Debug)]
pub enum Effects {
    NOP(),
    //An effect wrapped in parenthesis, just a wrapper around the effect.
    Paren(Box<Effects>),
    //Creates a variable
    CreateVariable(String, Box<Effects>),
    //Label of jumping to body
    Jump(String),
    //Comparison effect, and label to jump to the first if true, second if false
    CompareJump(Box<Effects>, String, String),
    CodeBody(CodeBody),
    //Calling, trait to call, function name, args, and return type (if explicitly required)
    ImplementationCall(Box<Effects>, String, String, Vec<Effects>, Option<UnparsedType>),
    //Calling, calling function, function arguments, and return type (if explicitly required)
    MethodCall(Option<Box<Effects>>, String, Vec<Effects>, Option<UnparsedType>),
    //Sets pointer to value
    Set(Box<Effects>, Box<Effects>),
    //Loads variable
    LoadVariable(String),
    //Loads field pointer from structure
    Load(Box<Effects>, String),
    //An unresolved operation, sent to the checker.
    Operation(String, Vec<Effects>),
    //Struct to create and a tuple of the index of the argument and the argument
    CreateStruct(UnparsedType, Vec<(String, Effects)>),
    CreateArray(Vec<Effects>),
    Float(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
    String(String)
}

#[derive(Clone, Debug)]
pub enum FinalizedEffects {
    //Exclusively used for void returns.
    NOP(),
    //Creates a variable
    CreateVariable(String, Box<FinalizedEffects>, FinalizedTypes),
    //Label of jumping to body
    Jump(String),
    //Comparison effect, and label to jump to the first if true, second if false
    CompareJump(Box<FinalizedEffects>, String, String),
    CodeBody(FinalizedCodeBody),
    //Output pointer, calling and function arguments
    MethodCall(Option<Box<FinalizedEffects>>, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    //Sets pointer to value
    Set(Box<FinalizedEffects>, Box<FinalizedEffects>),
    //Loads variable
    LoadVariable(String),
    //Loads field pointer from structure, with the given struct
    Load(Box<FinalizedEffects>, String, Arc<FinalizedStruct>),
    //Where to put the struct, struct to create and a tuple of the index of the argument and the argument
    CreateStruct(Option<Box<FinalizedEffects>>, FinalizedTypes, Vec<(usize, FinalizedEffects)>),
    //Create an array with the type and values
    CreateArray(Option<FinalizedTypes>, Vec<FinalizedEffects>),
    Float(f64),
    UInt(u64),
    Bool(bool),
    String(String),
    //Calls a virtual method
    VirtualCall(usize, Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    //Downcasts a structure into its trait
    Downcast(Box<FinalizedEffects>, FinalizedTypes),
    //Internally used by low-level verifier
    HeapStore(Box<FinalizedEffects>),
    //Allocates space
    HeapAllocate(FinalizedTypes),
    PointerLoad(Box<FinalizedEffects>),
    StackStore(Box<FinalizedEffects>)
}

impl FinalizedEffects {
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
                        FinalizedTypes::Generic(name, _) => {
                            panic!("Unresolved generic {}", name)
                        }
                        FinalizedTypes::GenericType(name, _) => {
                            panic!("Unresolved generic {:?}", name)
                        }
                        _ => return Some(found)
                    }
                }
                panic!("Unresolved variable {} from {:?}", name, variables);
            }
            FinalizedEffects::Load(_, name, loading) =>
                loading.fields.iter()
                    .find(|field| &field.field.name == name)
                    .map(|field| field.field.field_type.clone()),
            FinalizedEffects::CreateStruct(_, types, _) => Some(FinalizedTypes::Reference(Box::new(types.clone()))),
            FinalizedEffects::Float(_) => Some(FinalizedTypes::Struct(F64.clone())),
            FinalizedEffects::UInt(_) => Some(FinalizedTypes::Struct(U64.clone())),
            FinalizedEffects::Bool(_) => Some(FinalizedTypes::Struct(BOOL.clone())),
            FinalizedEffects::String(_) => Some(FinalizedTypes::Struct(STR.clone())),
            FinalizedEffects::HeapStore(inner) => inner.get_return(variables),
            FinalizedEffects::StackStore(inner) => inner.get_return(variables),
            FinalizedEffects::PointerLoad(inner) => match inner.get_return(variables).unwrap() {
                FinalizedTypes::Reference(inner) => Some(*inner),
                _ => panic!("Tried to load non-reference!")
            },
            FinalizedEffects::HeapAllocate(_) => panic!("Tried to return type a heap allocation!"),
            FinalizedEffects::CreateArray(types, _) =>
                types.clone().map(|inner| FinalizedTypes::Array(Box::new(inner))),
            FinalizedEffects::Downcast(_, target) => Some(target.clone())
        };
        return temp;
    }

    #[async_recursion]
    pub async fn degeneric(&mut self, process_manager: &Box<dyn ProcessManager>, variables: &mut SimpleVariableManager, syntax: &Arc<Mutex<Syntax>>) -> Result<(), ParsingError> {
        match self {
            FinalizedEffects::NOP() => {}
            FinalizedEffects::CreateVariable(_, first, other) => {
                first.degeneric(process_manager, variables, syntax).await?;
                other.degeneric(process_manager.generics(), syntax, ParsingError::empty(), ParsingError::empty()).await?;
            },
            FinalizedEffects::Jump(_) => {}
            FinalizedEffects::CompareJump(comparing, _, _) => comparing.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::CodeBody(body) => {
                for statement in &mut body.expressions {
                    statement.effect.degeneric(process_manager, variables, syntax).await?;
                }
            }
            FinalizedEffects::MethodCall(calling, method, effects) => {
                if let Some(inner) = calling {
                    inner.degeneric(process_manager, variables, syntax).await?;
                }
                for effect in &mut *effects {
                    effect.degeneric(process_manager, variables, syntax).await?;
                }
                let manager: Box<dyn ProcessManager> = process_manager.cloned();
                *method = CodelessFinalizedFunction::degeneric(method.clone(), manager, effects, syntax, variables, None).await?;
            }
            // Virtual calls can't be generic
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
            FinalizedEffects::HeapStore(storing) => storing.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::HeapAllocate(other) =>
                other.degeneric(process_manager.generics(), syntax, ParsingError::empty(), ParsingError::empty()).await?,
            FinalizedEffects::PointerLoad(loading) => loading.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::StackStore(storing) => storing.degeneric(process_manager, variables, syntax).await?,
            FinalizedEffects::Downcast(_, _) => {}
        }
        return Ok(());
    }

    pub fn is_constant(&self, _variables: &dyn VariableManager) -> bool {
        return match self {
            FinalizedEffects::Float(_) | FinalizedEffects::Bool(_) | FinalizedEffects::String(_) => true,
            _ => false
        }
    }
}