use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::{DisplayIndented, to_modifiers};
use crate::blocks::IfStatement;
use crate::function::{Arguments, CodeBody, display, Function};
use crate::type_resolver::TypeResolver;
use crate::types::Types;

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
    pub field_type: Rc<Types>
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
    pub fn new(name: String, field_type: Rc<Types>) -> Self {
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
        if self.effect.unwrap().has_return() {
            return write!(f, ";\n");
        }
        return write!(f, "\n");
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect: DisplayIndented {
    fn is_return(&self) -> bool;

    fn has_return(&self) -> bool;

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<Rc<Types>>;

    fn get_location(&self) -> (u32, u32);
}

pub enum Effects {
    NOP(),
    Wrapped(Box<Effects>),
    CodeBody(Box<CodeBody>),
    IfStatement(Box<IfStatement>),
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
            Effects::CodeBody(effect) => effect.as_ref(),
            Effects::IfStatement(effect) => effect.as_ref(),
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

    fn has_return(&self) -> bool {
        return true;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
        let mut output = Vec::new();
        for arg in &self.arguments.arguments {
            output.push(arg);
        }
        return type_resolver.get_method_type(&self.method, &self.calling, &output);
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

    fn has_return(&self) -> bool {
        return true;
    }

    fn return_type(&self, _type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
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
    fn get_type() -> &'static str;
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

impl<T> Effect for NumberEffect<T> where T : Display + Typed {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
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

    fn has_return(&self) -> bool {
        return true;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
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
    pub effects: Vec<Effects>,
    pub priority: i8,
    pub parse_left: bool,
    parent_function: String,
    location: (u32, u32)
}

impl OperatorEffect {
    pub fn new(function: &Function, effects: Vec<Effects>, location: (u32, u32)) -> Self {
        return Self {
            operator: function.name.clone(),
            effects,
            priority: function.attributes.get("priority")
                .map_or(0, |attrib| attrib.value.parse().expect("Expected numerical priority!")),
            parse_left: function.attributes.get("parse_left")
                .map_or(true, |attrib| attrib.value.parse().expect("Expected boolean parse_left!")),
            parent_function: function.name.clone(),
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

    fn has_return(&self) -> bool {
        return true;
    }

    fn return_type(&self, type_resolver: &dyn TypeResolver) -> Option<Rc<Types>> {
        let mut args = Vec::new();
        for arg in &self.effects {
            args.push(arg);
        }
        return type_resolver.get_method_type(&self.parent_function, &None, &args);
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location
    }
}

impl DisplayIndented for OperatorEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut skipping = false;
        let mut effects = self.effects.iter();
        //Get the operation itself
        for char in self.operator.split("::").last().unwrap().chars() {
            //Replace placeholders
            if skipping {
                skipping = false;
                effects.next().unwrap().format(indent, f)?;
            } else if char == '{' {
                skipping = true;
            }  else {
                write!(f, "{}", char)?;
            }
        }

        return Ok(());
    }
}