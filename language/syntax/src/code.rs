use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{Attribute, DisplayIndented, ErrorProvider, Function, ProcessManager, to_modifiers, VariableManager};
use crate::function::{CodeBody, display_indented, display_joined};
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

#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub field_type: Types,
}

#[derive(Clone)]
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
    pub fn get_return(&self, process_manager: &dyn ProcessManager, variables: &dyn VariableManager) -> Option<Types> {
        return match self {
            Effects::NOP() => None,
            Effects::Jump(_) => None,
            Effects::CompareJump(_, _, _) => None,
            Effects::CodeBody(_) => None,
            Effects::Operation(_, _) => panic!("Failed to resolve operation?"),
            Effects::MethodCall(function, _) => function.return_type.clone(),
            Effects::Set(_, to) => to.get_return(process_manager, variables),
            Effects::LoadVariable(name) => variables.get_variable(name),
            Effects::Load(from, _) => from.get_return(process_manager, variables),
            Effects::CreateStruct(types, _) => Some(types.clone()),
            Effects::Float(_) => Some(Types::Struct(process_manager.get_internal("f64"))),
            Effects::Int(_) => Some(Types::Struct(process_manager.get_internal("i64"))),
            Effects::UInt(_) => Some(Types::Struct(process_manager.get_internal("u64"))),
            Effects::String(_) => Some(Types::Reference(Box::new(Types::Struct(process_manager.get_internal("str")))))
        };
    }
}

impl DisplayIndented for Effects {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let deeper = parsing.to_string() + "    ";
        return match self {
            Effects::NOP() => Ok(()),
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