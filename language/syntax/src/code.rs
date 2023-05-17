use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{Attribute, DisplayIndented, Function, to_modifiers, VariableManager};
use crate::function::{CodeBody, display_indented, display_joined};
use crate::r#struct::{F64, I64, STR, U64};
use crate::types::Types;

#[derive(Clone, Debug)]
pub struct Expression {
    pub expression_type: ExpressionType,
    pub effect: Effects,
}

#[derive(Clone, Copy, Debug)]
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
pub struct MemberField {
    pub modifiers: u8,
    pub attributes: Vec<Attribute>,
    pub field: Field,
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

impl Field {
    pub fn new(name: String, field_type: Types) -> Self {
        return Self {
            name,
            field_type,
        };
    }
}

impl DisplayIndented for Expression {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", indent)?;
        match self.expression_type {
            ExpressionType::Return => write!(f, "return")?,
            ExpressionType::Break => write!(f, "break")?,
            _ => {}
        }
        if let ExpressionType::Line = self.expression_type {
            //Only add a space for returns
        } else {
            write!(f, " ")?;
        }
        self.effect.format(indent, f)?;
        return write!(f, "\n");
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
    //Calling function, function arguments
    MethodCall(Arc<Function>, Vec<Effects>),
    //Sets pointer to value
    Set(Box<Effects>, Box<Effects>),
    //Loads variable
    LoadVariable(String),
    //Loads field pointer from structure
    Load(Box<Effects>, String),
    //An unresolved operation, sent to the checker.
    Operation(String, Vec<Effects>),
    //Struct to create and a tuple of the index of the argument and the argument
    CreateStruct(Types, Vec<(usize, Effects)>),
    Float(f64),
    Int(i64),
    UInt(u64),
    String(String),
}

impl Effects {
    pub fn get_return(&self, variables: &dyn VariableManager) -> Option<Types> {
        return match self {
            Effects::NOP() => None,
            Effects::Jump(_) => None,
            Effects::CompareJump(_, _, _) => None,
            Effects::CodeBody(_) => None,
            Effects::CreateVariable(_, effect) => effect.get_return(variables),
            Effects::Operation(_, _) => panic!("Failed to resolve operation?"),
            Effects::MethodCall(function, _) => function.return_type.clone(),
            Effects::Set(_, to) => to.get_return(variables),
            Effects::LoadVariable(name) => {
                let variable = variables.get_variable(name);
                if let Some(found) = variable {
                    match found {
                        Types::Generic(name, _) => {
                            panic!("Unresolved generic {}", name)
                        }
                        Types::GenericType(name, _) => {
                            panic!("Unresolved generic {:?}", name)
                        }
                        _ => return Some(found)
                    }
                }
                panic!("Unresolved variable {}", name);
            },
            Effects::Load(from, name) =>
                match from.get_return(variables).unwrap() {
                    Types::Struct(structure) => {
                        structure.fields.iter()
                            .find(|field| &field.field.name == name)
                            .map(|field| field.field.field_type.clone())
                    },
                    _ => None
                }
            Effects::CreateStruct(types, _) => Some(types.clone()),
            Effects::Float(_) => Some(Types::Struct(F64.clone())),
            Effects::Int(_) => Some(Types::Struct(I64.clone())),
            Effects::UInt(_) => Some(Types::Struct(U64.clone())),
            Effects::String(_) => Some(Types::Reference(Box::new(Types::Struct(STR.clone()))))
        };
    }
}

impl DisplayIndented for Effects {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let deeper = parsing.to_string() + "    ";
        return match self {
            Effects::NOP() => Ok(()),
            Effects::CreateVariable(name, effect) => {
                write!(f, "{} = ", name)?;
                return effect.format(&deeper, f);
            },
            Effects::Operation(operation, effects) => {
                let mut index = 0;
                let mut effect = 0;
                while index < operation.len() {
                    if operation.as_bytes()[index] == b'{' {
                        index += 1;
                        effects.get(effect).unwrap().format(&deeper, f)?;
                        effect += 1;
                    } else {
                        write!(f, "{}", operation.as_bytes()[index])?;
                    }
                    index += 1;
                }
                return Ok(());
            }
            Effects::Jump(label) => write!(f, "jump {}", label),
            Effects::CompareJump(comparing, label, other) => {
                write!(f, "if ")?;
                comparing.format(&deeper, f)?;
                write!(f, " jump {} else {}", label, other)
            },
            Effects::LoadVariable(variable) => write!(f, "{}", variable),
            Effects::CodeBody(body) => body.format(&deeper, f),
            Effects::MethodCall(function, args) => {
                write!(f, "{}.", function.name, )?;
                display_indented(f, args, &deeper, ", ")
            }
            Effects::Set(setting, value) => {
                setting.format(&deeper, f)?;
                write!(f, " = ")?;
                value.format(&deeper, f)
            }
            Effects::Load(from, loading) => {
                from.format(&deeper, f)?;
                write!(f, ".{}", loading)
            }
            Effects::CreateStruct(structure, arguments) => {
                write!(f, "{} {{", structure)?;
                for (_, arg) in arguments {
                    arg.format(&deeper, f)?;
                }
                write!(f, "}}")
            }
            Effects::Float(float) => write!(f, "{}", float),
            Effects::Int(int) => write!(f, "{}", int),
            Effects::UInt(uint) => write!(f, "{}", uint),
            Effects::String(string) => write!(f, "{}", string)
        };
    }
}