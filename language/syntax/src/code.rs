use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{Attribute, DisplayIndented, to_modifiers, VariableManager};
use crate::function::{CodeBody, display_joined, FinalizedCodeBody, CodelessFinalizedFunction};
use crate::r#struct::{BOOL, F64, FinalizedStruct, I64, STR, U64};
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
    CreateStruct(Types, Vec<(String, Effects)>),
    Float(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
    String(String),
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
    //Calling and function arguments
    MethodCall(Arc<CodelessFinalizedFunction>, Vec<FinalizedEffects>),
    //Sets pointer to value
    Set(Box<FinalizedEffects>, Box<FinalizedEffects>),
    //Loads variable
    LoadVariable(String),
    //Loads field pointer from structure, with the given struct
    Load(Box<FinalizedEffects>, String, Arc<FinalizedStruct>),
    //Struct to create and a tuple of the index of the argument and the argument
    CreateStruct(FinalizedTypes, Vec<(usize, FinalizedEffects)>),
    Float(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
    String(String),
}

impl FinalizedEffects {
    pub fn get_return(&self, variables: &dyn VariableManager) -> Option<FinalizedTypes> {
        let temp = match self {
            FinalizedEffects::NOP() => None,
            FinalizedEffects::Jump(_) => None,
            FinalizedEffects::CompareJump(_, _, _) => None,
            FinalizedEffects::CodeBody(_) => None,
            FinalizedEffects::CreateVariable(_, _, types) => Some(types.clone()),
            FinalizedEffects::MethodCall(function, _) =>
                function.return_type.as_ref().map(|inner| {
                    let mut returning = inner.clone();
                    if !returning.is_primitive() {
                        returning = FinalizedTypes::Reference(Box::new(returning));
                    }
                    returning
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
                panic!("Unresolved variable {}", name);
            }
            FinalizedEffects::Load(_, name, loading) =>
                loading.fields.iter()
                    .find(|field| &field.field.name == name)
                    .map(|field| field.field.field_type.clone()),
            FinalizedEffects::CreateStruct(types, _) => Some(FinalizedTypes::Reference(Box::new(types.clone()))),
            FinalizedEffects::Float(_) => Some(FinalizedTypes::Struct(F64.clone())),
            FinalizedEffects::Int(_) => Some(FinalizedTypes::Struct(I64.clone())),
            FinalizedEffects::UInt(_) => Some(FinalizedTypes::Struct(U64.clone())),
            FinalizedEffects::Bool(_) => Some(FinalizedTypes::Struct(BOOL.clone())),
            FinalizedEffects::String(_) => Some(FinalizedTypes::Reference(Box::new(FinalizedTypes::Struct(STR.clone()))))
        };
        return temp;
    }
}