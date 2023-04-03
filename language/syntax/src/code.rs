use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{DisplayIndented, Function, Struct, to_modifiers};
use crate::function::{CodeBody, display, display_joined};
use crate::types::Types;

#[derive(Clone)]
pub struct Expression {
    pub expression_type: ExpressionType,
    pub effect: Effects,
}

#[derive(Clone, Copy)]
pub enum ExpressionType {
    Break,
    Return,
    Line,
}

#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub field_type: Arc<Types>,
}

#[derive(Clone)]
pub struct MemberField {
    pub modifiers: u8,
    pub field: Field,
}

impl MemberField {
    pub fn new(modifiers: u8, field: Field) -> Self {
        return Self {
            modifiers,
            field,
        };
    }
}

impl DisplayIndented for MemberField {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{} {};", indent, display_joined(&to_modifiers(self.modifiers)), self.field);
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
    pub fn new(name: String, field_type: Arc<Types>) -> Self {
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
        return write!(f, "{}: {}", self.name, self.field_type.name);
    }
}

#[derive(Clone)]
pub enum Effects {
    NOP(),
    //Label of jumping to body
    Jump(String),
    CodeBody(CodeBody),
    //Calling function, function arguments
    MethodCall(Arc<Function>, Effects),
    //Sets pointer to value
    Set(Effects, Effects),
    //Loads variable/field pointer from structure, or self if structure is None
    Load(Option<Effects>, String),
    //Struct to create and a tuple of the index of the argument and the argument
    CreateStruct(Types, Vec<(usize, Effects)>),
    Float(f64),
    Int(i64),
    UInt(u64),
    String(String)
}

impl DisplayIndented for Effects {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let deeper = parsing.to_string() + "    ";
        return match self {
            Effects::NOP() => Ok(()),
            Effects::Jump(label) => write!(f, "jump {}", label),
            Effects::CodeBody(body) => body.format(&deeper, f),
            Effects::MethodCall(function, args) =>
                write!(f, "{}.{}", function.name, display(args, ", ")),
            Effects::Set(setting, value) => {
                setting.format(&deeper, f)?;
                write!(f, " = ")?;
                value.format(&deeper, f)
            },
            Effects::Load(from, loading) => {
                from.format(&deeper, f)?;
                write!(".loading")
            },
            Effects::CreateStruct(structure, arguments) => {
                write!(f, "{} {{", structure.name)?;
                for (_, arg) in arguments {
                    arg.format(&deeper, f)?;
                }
                write!(f, "}}")
            },
            Effects::Float(float) => write!(f, "{}", float),
            Effects::Int(int) => write!(f, "{}", int),
            Effects::UInt(uint) => write!(f, "{}", uint),
            Effects::String(string) => write!(f, "{}", string)
        }
    }
}