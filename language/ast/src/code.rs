use std::fmt::{Display, Formatter, Pointer};
use crate::{DisplayIndented, to_modifiers};
use crate::function::{Arguments, display, Function};
use crate::type_resolver::TypeResolver;

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

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<String>;

    fn get_location(&self) -> (u32, u32);
}

pub enum Effects {
    NOP(),
    Wrapped(Box<Effects>),
    MethodCall(Box<MethodCall>),
    VariableLoad(Box<VariableLoad>),
    FloatEffect(Box<NumberEffect<f64>>),
    IntegerEffect(Box<NumberEffect<i64>>),
    AssignVariable(Box<AssignVariable>),
    OperatorEffect(Box<OperatorEffect>)
}

impl Effects {
    pub fn unwrap(&self) -> &dyn Effect {
        return match self {
            Effects::NOP() => panic!("Tried to unwrap a NOP!"),
            Effects::Wrapped(effect) => effect.unwrap(),
            Effects::MethodCall(effect) => effect.as_ref(),
            Effects::VariableLoad(effect) => effect.as_ref(),
            Effects::FloatEffect(effect) => effect.as_ref(),
            Effects::IntegerEffect(effect) => effect.as_ref(),
            Effects::AssignVariable(effect) => effect.as_ref(),
            Effects::OperatorEffect(effect) => effect.as_ref()
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
        return match self {
            Effects::Wrapped(effect) => {
                write!(f, "(")?;
                effect.format(indent, f)?;
                write!(f, ")")
            },
            Effects::NOP() => write!(f, "{{}}"),
            _ => self.unwrap().format(indent, f)
        }
    }
}

pub struct MethodCall {
    pub calling: Option<Effects>,
    pub method: String,
    pub arguments: Arguments,
    location: (u32, u32)
}

impl MethodCall {
    pub fn new(calling: Option<Effects>, method: String, arguments: Arguments, location: (u32, u32)) -> Self {
        return Self {
            calling,
            method,
            arguments,
            location
        };
    }
}

impl Effect for MethodCall {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<String> {
        return type_resolver.get_method_type(&self.method, &self.calling, &self.arguments);
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}

impl DisplayIndented for MethodCall {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(value) = &self.calling {
            value.format(indent, f)?;
            write!(f, ".")?;
        }
        return write!(f, "{}{}", self.method, self.arguments);
    }
}

pub struct VariableLoad {
    pub name: String,
    location: (u32, u32)
}

impl VariableLoad {
    pub fn new(name: String, location: (u32, u32)) -> Self {
        return Self {
            name,
            location
        }
    }
}

impl Effect for VariableLoad {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self, _type_resolver: &dyn TypeResolver) -> Option<String> {
        return None;
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }
}

impl DisplayIndented for VariableLoad {
    fn format(&self, _indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
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

    fn return_type(&self, _type_resolver: &dyn TypeResolver) -> Option<String> {
        return Some(T::get_type());
    }

    fn get_location(&self) -> (u32, u32) {
        panic!("Unexpected get location!");
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
    pub effect: Effects,
    location: (u32, u32)
}

impl AssignVariable {
    pub fn new(variable: String, given_type: Option<String>, effect: Effects, location: (u32, u32)) -> Self {
        return Self {
            variable,
            given_type,
            effect,
            location
        }
    }
}

impl Effect for AssignVariable {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<String> {
        return self.effect.unwrap().return_type(type_resolver);
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
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

pub struct OperatorEffect {
    pub operator: String,
    pub operator_symbol: String,
    pub lhs: Option<Effects>,
    pub rhs: Option<Effects>,
    pub priority: i8,
    pub parse_left: bool,
    return_type: Option<String>,
    location: (u32, u32)
}

impl OperatorEffect {
    pub fn new(operator: &String, function: &Function, lhs: Option<Effects>, rhs: Option<Effects>, location: (u32, u32)) -> Self {
        return Self {
            operator: function.name.clone(),
            operator_symbol: operator.clone(),
            lhs,
            rhs,
            priority: function.attributes.get("priority")
                .map_or(0, |attrib| attrib.value.parse().expect("Expected numerical priority!")),
            parse_left: function.attributes.get("parse_left")
                .map_or(true, |attrib| attrib.value.parse().expect("Expected boolean parse_left!")),
            return_type: function.return_type.clone(),
            location
        }
    }
}

impl Display for OperatorEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl Effect for OperatorEffect {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self, _type_resolver: &dyn TypeResolver) -> Option<String> {
        return self.return_type.clone();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}

impl DisplayIndented for OperatorEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match &self.lhs {
            Some(lhs) => match &self.rhs {
                Some(rhs) => {
                    lhs.format(indent, f)?;
                    write!(f, " {} ", self.operator_symbol)?;
                    rhs.format(indent, f)
                },
                None => {
                    lhs.format(indent, f)?;
                    write!(f, "{}", self.operator_symbol)
                }
            }
            None => {
                write!(f, "{}", self.operator_symbol)?;
                self.rhs.as_ref().unwrap().format(indent, f)
            }
        }
    }
}