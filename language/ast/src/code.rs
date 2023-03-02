use std::fmt::{Display, Formatter};
use std::mem;
use crate::{DisplayIndented, to_modifiers};
use crate::function::{Arguments, display};
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

    fn swap(&mut self, left: bool, swapping: &mut Effects);

    fn priority(&self) -> i8;

    fn parse_left_first(&self) -> bool;

    fn get_location(&self) -> (u32, u32);
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

    pub fn unwrap_mut(&mut self) -> &mut dyn Effect {
        return match self {
            Effects::NOP() => panic!("Tried to unwrap a NOP!"),
            Effects::MethodCall(effect) => effect.as_mut(),
            Effects::VariableLoad(effect) => effect.as_mut(),
            Effects::MathEffect(effect) => effect.as_mut(),
            Effects::FloatEffect(effect) => effect.as_mut(),
            Effects::IntegerEffect(effect) => effect.as_mut(),
            Effects::AssignVariable(effect) => effect.as_mut()
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
    pub effect: Option<Effects>,
    is_break: bool,
    location: (u32, u32)
}

impl ReturnEffect {
    pub fn new(effect: Option<Effects>, is_break: bool, location: (u32, u32)) -> Self {
        return Self {
            effect,
            is_break,
            location
        }
    }
}

impl Effect for ReturnEffect {
    fn is_return(&self) -> bool {
        return true;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<String> {
        return match &self.effect {
            Some(value) => value.unwrap().return_type(type_resolver),
            None => Some("void".to_string())
        }
    }

    fn swap(&mut self, _left: bool, _swapping: &mut Effects) {
        panic!("Unexpected reconstruct!");
    }

    fn priority(&self) -> i8 {
        panic!("Unexpected priority!");
    }

    fn parse_left_first(&self) -> bool {
        panic!("Unexpected parse left!");
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }
}

impl DisplayIndented for ReturnEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match &self.effect {
            Some(value) => {
                if self.is_break {
                    write!(f, "break ")
                } else {
                    write!(f, "return ")
                }?;
                value.format(indent, f)
            },
            None => if self.is_break {
                write!(f, "break")
            } else {
                write!(f, "return")
            }
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

    fn swap(&mut self, _left: bool, _swapping: &mut Effects) {
        panic!("Unexpected priority!");
    }

    fn priority(&self) -> i8 {
        0
    }

    fn parse_left_first(&self) -> bool {
        panic!("Unexpected parse left first!");
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

    fn swap(&mut self, _left: bool, _swapping: &mut Effects) {
        panic!("Unexpected reconstruct!");
    }

    fn priority(&self) -> i8 {
        panic!("Unexpected priority!");
    }

    fn parse_left_first(&self) -> bool {
        panic!("Unexpected parse left!");
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

pub struct MathEffect {
    pub target: Option<Effects>,
    pub operator: MathOperator,
    pub effect: Effects,
    location: (u32, u32)
}

#[derive(Copy, Clone)]
pub enum MathOperator {
    PLUS = '+' as isize,
    MINUS = '-' as isize,
    DIVIDE = '/' as isize,
    MULTIPLY = '*' as isize
}

impl MathEffect {
    pub fn new(target: Option<Effects>, operator: MathOperator, effect: Effects, location: (u32, u32)) -> Self {
        return Self {
            target,
            operator,
            effect,
            location
        }
    }
}

impl Effect for MathEffect {
    fn is_return(&self) -> bool {
        return false;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<String> {
        return self.effect.unwrap().return_type(type_resolver);
    }

    fn swap(&mut self, left: bool, swapping: &mut Effects) {
        if left {
            mem::swap(self.target.as_mut().unwrap(), swapping);
        } else {
            mem::swap(&mut self.effect, swapping);
        }
    }

    fn priority(&self) -> i8 {
        match self.operator {
            MathOperator::PLUS => 0,
            MathOperator::MINUS => 0,
            MathOperator::MULTIPLY => 1,
            MathOperator::DIVIDE => 1
        }
    }

    fn parse_left_first(&self) -> bool {
        true
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
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

    fn return_type(&self, _type_resolver: &dyn TypeResolver) -> Option<String> {
        return Some(T::get_type());
    }

    fn swap(&mut self, _left: bool, _swapping: &mut Effects) {
        panic!("Unexpected reconstruct!");
    }

    fn priority(&self) -> i8 {
        panic!("Unexpected priority!");
    }

    fn parse_left_first(&self) -> bool {
        panic!("Unexpected parse left!");
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

    fn swap(&mut self, _left: bool, _swapping: &mut Effects) {
        panic!("Unexpected reconstruct!");
    }

    fn priority(&self) -> i8 {
        panic!("Unexpected priority!");
    }

    fn parse_left_first(&self) -> bool {
        panic!("Unexpected parse left!");
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