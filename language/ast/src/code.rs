use std::fmt::{Display, Formatter};
use crate::{DisplayIndented, to_modifiers};
use crate::function::{Arguments, display, Function};
use crate::type_resolver::TypeResolver;
use crate::types::Types;

pub struct Expression<'a> {
    pub expression_type: ExpressionType,
    pub effect: Effects<'a>
}

#[derive(Clone, Copy)]
pub enum ExpressionType {
    Break,
    Return,
    Line
}

pub struct Field<'a> {
    pub name: String,
    pub field_type: &'a Types<'a>
}

pub struct MemberField<'a> {
    pub modifiers: u8,
    pub field: Field<'a>
}

impl<'a> DisplayIndented for MemberField<'a> {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{} {}", indent, display(&to_modifiers(self.modifiers)), self.field);
    }
}

impl<'a> Expression<'a> {
    pub fn new(expression_type: ExpressionType, effect: Effects<'a>) -> Self {
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

impl<'a> Field<'a> {
    pub fn new(name: String, field_type: &'a Types<'a>) -> Self {
        return Self {
            name,
            field_type
        }
    }
}

impl<'a> DisplayIndented for Expression<'a> {
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

impl<'a> Display for Field<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect<'a>: DisplayIndented {
    fn is_return(&self) -> bool;

    fn return_type(&'a self, type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>>;

    fn get_location(&self) -> (u32, u32);
}

pub enum Effects<'a> {
    NOP(),
    Wrapped(Box<Effects<'a>>),
    MethodCall(Box<MethodCall<'a>>),
    VariableLoad(Box<VariableLoad>),
    FloatEffect(Box<NumberEffect<f64>>),
    IntegerEffect(Box<NumberEffect<i64>>),
    AssignVariable(Box<AssignVariable<'a>>),
    OperatorEffect(Box<OperatorEffect<'a>>)
}

impl<'a> Effects<'a> {
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

impl<'a> Display for Effects<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl<'a> DisplayIndented for Effects<'a> {
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

pub struct MethodCall<'a> {
    pub calling: Option<Effects<'a>>,
    pub method: String,
    pub arguments: Arguments<'a>,
    location: (u32, u32)
}

impl<'a> MethodCall<'a> {
    pub fn new(calling: Option<Effects<'a>>, method: String, arguments: Arguments<'a>, location: (u32, u32)) -> Self {
        return Self {
            calling,
            method,
            arguments,
            location
        };
    }
}

impl<'a> Effect<'a> for MethodCall<'a> {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&'a self, type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>> {
        return type_resolver.get_method_type(&self.method, &self.calling, &self.arguments);
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}

impl<'a> DisplayIndented for MethodCall<'a> {
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

impl<'a> Effect<'a> for VariableLoad {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&'a self, _type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>> {
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
    fn get_type<'a>() -> &'static str;
}

impl Typed for f64 {
    fn get_type() -> &'static str {
        return "f64";
    }
}

impl Typed for i64 {
    fn get_type() -> &'static str {
        return "i64";
    }
}

impl<'a, T> Effect<'a> for NumberEffect<T> where T : Display + Typed {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&'a self, type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>> {
        return type_resolver.get_type(&T::get_type().to_string());
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

pub struct AssignVariable<'a> {
    pub variable: String,
    pub given_type: Option<String>,
    pub effect: Effects<'a>,
    location: (u32, u32)
}

impl<'a> AssignVariable<'a> {
    pub fn new(variable: String, given_type: Option<String>, effect: Effects<'a>, location: (u32, u32)) -> Self {
        return Self {
            variable,
            given_type,
            effect,
            location
        }
    }
}

impl<'a> Effect<'a> for AssignVariable<'a> {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&'a self, type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>> {
        return self.effect.unwrap().return_type(type_resolver);
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }
}

impl<'a> DisplayIndented for AssignVariable<'a> {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "let {}", self.variable)?;
        if self.given_type.is_some() {
            write!(f, ": {}", self.given_type.as_ref().unwrap())?;
        }
        write!(f, " = ")?;
        return self.effect.format(indent, f);
    }
}

pub struct OperatorEffect<'a> {
    pub operator: String,
    pub operator_symbol: String,
    pub lhs: Option<Effects<'a>>,
    pub rhs: Option<Effects<'a>>,
    pub priority: i8,
    pub parse_left: bool,
    return_type: Option<&'a Types<'a>>,
    location: (u32, u32)
}

impl<'a> OperatorEffect<'a> {
    pub fn new(operator: &String, function: &Function<'a>, lhs: Option<Effects<'a>>, rhs: Option<Effects<'a>>, location: (u32, u32)) -> Self {
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

impl<'a> Display for OperatorEffect<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl<'a> Effect<'a> for OperatorEffect<'a> {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&'a self, _type_resolver: &'a dyn TypeResolver) -> Option<&'a Types<'a>> {
        return self.return_type;
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}

impl<'a> DisplayIndented for OperatorEffect<'a> {
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