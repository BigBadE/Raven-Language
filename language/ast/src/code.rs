use std::fmt::{Display, Formatter};
use crate::{DisplayIndented, to_modifiers};
use crate::function::{Arguments, display};

pub struct Expression {
    pub expression_type: ExpressionType,
    pub effect: Effects
}

#[derive(Clone, Copy)]
pub enum ExpressionType {
    Break,
    Return,
    Line
}

pub struct Field {
    pub name: String,
    pub field_type: String
}

pub struct MemberField {
    pub modifiers: u8,
    pub field: Field
}

impl DisplayIndented for MemberField {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{} {}", indent, display(&to_modifiers(self.modifiers)), self.field);
    }
}

impl Expression {
    pub fn new(expression_type: ExpressionType, effect: Effects) -> Self {
        return Self {
            expression_type,
            effect
        }
    }

    pub fn is_return(&self) -> bool {
        return if let ExpressionType::Return = self.expression_type {
            true
        } else {
            self.effect.unwrap().is_return()
        }
    }
}

impl Field {
    pub fn new(name: String, field_type: String) -> Self {
        return Self {
            name,
            field_type
        }
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
        if let Effects::NOP() = self.effect {
            return write!(f, ";\n");
        } else if let ExpressionType::Line = self.expression_type {
            //Only add a space for returns
        } else {
            write!(f, " ")?;
        }
        self.effect.format(indent, f)?;
        return write!(f, ";\n");
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect: DisplayIndented {
    fn is_return(&self) -> bool;

    fn return_type(&self) -> Option<String>;
}

pub enum Effects {
    NOP(),
    MethodCall(Box<MethodCall>),
    VariableLoad(Box<VariableLoad>),
    MathEffect(Box<MathEffect>),
    FloatEffect(Box<NumberEffect<f64>>),
    IntegerEffect(Box<NumberEffect<i64>>),
    AssignVariable(Box<AssignVariable>)
}

impl Effects {
    pub fn unwrap(&self) -> &dyn Effect {
        return match self {
            Effects::NOP() => panic!("Tried to unwrap a NOP!"),
            Effects::MethodCall(effect) => effect.as_ref(),
            Effects::VariableLoad(effect) => effect.as_ref(),
            Effects::MathEffect(effect) => effect.as_ref(),
            Effects::FloatEffect(effect) => effect.as_ref(),
            Effects::IntegerEffect(effect) => effect.as_ref(),
            Effects::AssignVariable(effect) => effect.as_ref()
        };
    }
}

impl Display for Effects {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl DisplayIndented for Effects {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.unwrap().format(indent, f);
    }
}

pub struct ReturnEffect {
    pub effect: Option<Effects>
}

impl ReturnEffect {
    pub fn new(effect: Option<Effects>) -> Self {
        return Self {
            effect
        }
    }
}

pub struct BreakEffect {
    pub effect: Option<Effects>
}

impl BreakEffect {
    pub fn new(effect: Option<Effects>) -> Self {
        return Self {
            effect
        }
    }
}

impl Effect for BreakEffect {
    fn is_return(&self) -> bool {
        false
    }

    fn return_type(&self) -> Option<String> {
        return match &self.effect {
            Some(effect) => effect.unwrap().return_type(),
            None => Some("void".to_string())
        }
    }
}

impl DisplayIndented for BreakEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match &self.effect {
            Some(value) => {
                write!(f, "break ")?;
                value.format(indent, f)
            },
            None => write!(f, "break")
        }
    }
}

impl Effect for ReturnEffect {
    fn is_return(&self) -> bool {
        return true;
    }

    fn return_type(&self) -> Option<String> {
        return match &self.effect {
            Some(value) => value.unwrap().return_type(),
            None => Some("void".to_string())
        }
    }
}

impl DisplayIndented for ReturnEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match &self.effect {
            Some(value) => {
                write!(f, "return ")?;
                value.format(indent, f)
            },
            None => write!(f, "return")
        }
    }
}

pub struct MethodCall {
    pub calling: Effects,
    pub method: String,
    pub arguments: Arguments
}

impl MethodCall {
    pub fn new(calling: Effects, method: String, arguments: Arguments) -> Self {
        return Self {
            calling,
            method,
            arguments
        };
    }
}

impl Effect for MethodCall {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self) -> Option<String> {
        return self.calling.unwrap().return_type();
    }
}

impl DisplayIndented for MethodCall {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.calling.format(indent, f)?;
        return write!(f, ".{}{}", self.method, self.arguments);
    }
}

pub struct VariableLoad {
    pub name: String
}

impl VariableLoad {
    pub fn new(name: String) -> Self {
        return Self {
            name
        }
    }
}

impl Effect for VariableLoad {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self) -> Option<String> {
        return None;
    }
}

impl DisplayIndented for VariableLoad {
    fn format(&self, _indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
    }
}

pub struct MathEffect {
    pub target: Option<Effects>,
    pub operator: MathOperator,
    pub effect: Effects
}

#[derive(Copy, Clone)]
pub enum MathOperator {
    PLUS = '+' as isize,
    MINUS = '-' as isize,
    DIVIDE = '/' as isize,
    MULTIPLY = '*' as isize
}

impl MathEffect {
    pub fn new(target: Option<Effects>, operator: MathOperator, effect: Effects) -> Self {
        return Self {
            target,
            operator,
            effect
        }
    }
}

impl Effect for MathEffect {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self) -> Option<String> {
        return self.effect.unwrap().return_type();
    }
}

impl DisplayIndented for MathEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.target {
            Some(target) => {
                target.format(indent, f)?;
                write!(f, " {} ", self.operator.clone() as u8 as char)
            },
            None => write!(f, "{}", self.operator.clone() as u8 as char)
        }?;
        return self.effect.format(indent, f);
    }
}

pub struct NumberEffect<T> where T : Display {
    pub number: T
}

impl<T> NumberEffect<T> where T : Display {
    pub fn new(number: T) -> Self {
        return Self {
            number
        }
    }
}

pub trait Typed {
    fn get_type() -> String;
}

impl Typed for f64 {
    fn get_type() -> String {
        return "f64".to_string();
    }
}

impl Typed for i64 {
    fn get_type() -> String {
        return "i64".to_string();
    }
}

impl<T> Effect for NumberEffect<T> where T : Display + Typed {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self) -> Option<String> {
        return Some(T::get_type());
    }
}

impl<T> DisplayIndented for NumberEffect<T> where T : Display {
    fn format(&self, _indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.number);
    }
}

pub struct AssignVariable {
    pub variable: String,
    pub given_type: Option<String>,
    pub effect: Effects
}

impl AssignVariable {
    pub fn new(variable: String, given_type: Option<String>, effect: Effects) -> Self {
        return Self {
            variable,
            given_type,
            effect
        }
    }
}

impl Effect for AssignVariable {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self) -> Option<String> {
        return self.effect.unwrap().return_type();
    }
}

impl DisplayIndented for AssignVariable {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "let {}", self.variable)?;
        if self.given_type.is_some() {
            write!(f, ": {}", self.given_type.as_ref().unwrap())?;
        }
        write!(f, " = ")?;
        return self.effect.format(indent, f);
    }
}