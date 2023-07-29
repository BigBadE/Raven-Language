use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{Attribute, DisplayIndented, to_modifiers, VariableManager};
use crate::async_util::UnparsedType;
use crate::function::{CodeBody, display_joined, FinalizedCodeBody, CodelessFinalizedFunction};
use crate::r#struct::{BOOL, F64, FinalizedStruct, STR, U64};
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

impl Display for MemberField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return DisplayIndented::format(self, "", f);
    }
}

impl DisplayIndented for MemberField {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{} {}", indent, display_joined(&to_modifiers(self.modifiers)), self.field);
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
    //Creates a variable
    CreateVariable(String, Box<Effects>),
    //Label of jumping to body
    Jump(String),
    //Comparison effect, and label to jump to the first if true, second if false
    CompareJump(Box<Effects>, String, String),
    CodeBody(CodeBody),
    //Calling, trait to call, function name, args
    ImplementationCall(Box<Effects>, String, String, Vec<Effects>),
    //Calling, calling function, function arguments
    MethodCall(Option<Box<Effects>>, String, Vec<Effects>),
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
    Float(f64),
    UInt(u64),
    Bool(bool),
    String(String),
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
            FinalizedEffects::String(_) => Some(FinalizedTypes::Reference(Box::new(FinalizedTypes::Struct(STR.clone())))),
            FinalizedEffects::HeapStore(inner) => inner.get_return(variables),
            FinalizedEffects::StackStore(inner) => inner.get_return(variables),
            FinalizedEffects::PointerLoad(inner) => match inner.get_return(variables).unwrap() {
                FinalizedTypes::Reference(inner) => Some(*inner),
                _ => panic!("Tried to load non-reference!")
            },
            FinalizedEffects::HeapAllocate(_) => panic!("Tried to return type a heap allocation!")
        };
        return temp;
    }

    pub fn is_constant(&self, _variables: &dyn VariableManager) -> bool {
        return match self {
            FinalizedEffects::Float(_) | FinalizedEffects::Bool(_) | FinalizedEffects::String(_) => true,
            _ => false
        }
    }
}