use std::convert::Infallible;
use std::ops::{ControlFlow, FromResidual, Try};

pub struct Token {
    pub token_type: TokenTypes,
    pub start: usize,
    pub end: usize
}

impl Token {
    pub fn new(token_type: TokenTypes, start: usize, end: usize) -> Self {
        return Self {
            token_type,
            start,
            end
        }
    }
}

impl Try for Token {
    type Output = Token;
    type Residual = Token;

    fn from_output(output: Self::Output) -> Self {
        return output;
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        return ControlFlow::Continue(self);
    }
}

impl FromResidual<Token> for Token {
    fn from_residual(residual: Token) -> Self {
        return residual;
    }
}

impl FromResidual<Result<Infallible, Token>> for Token {
    fn from_residual(residual: Result<Infallible, Token>) -> Token {
        return residual.err().unwrap();
    }
}

#[derive(PartialEq)]
pub enum TokenTypes {
    Start,
    EOF,
    InvalidCharacters,
    StringStart,
    StringEscape,
    StringEnd,
    ImportStart,
    Identifier,
    AttributesStart,
    Attribute,
    ModifiersStart,
    Modifier,
    ElemStart,
    GenericsStart,
    Generic,
    GenericBound,
    ArgumentsStart,
    ArgumentName,
    ArgumentType,
    ArgumentEnd,
    ArgumentsEnd,
    ReturnTypeStart,
    ReturnType,
    CodeStart
}