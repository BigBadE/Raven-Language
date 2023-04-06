use std::convert::Infallible;
use std::ops::{ControlFlow, FromResidual, Try};

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum TokenTypes {
    Start = 0,
    EOF = 1,
    InvalidCharacters = 2,
    StringStart = 3,
    StringEscape = 4,
    StringEnd = 5,
    ImportStart = 6,
    Identifier = 7,
    AttributesStart = 8,
    Attribute = 9,
    ModifiersStart = 10,
    Modifier = 11,
    GenericsStart = 12,
    Generic = 13,
    GenericBound = 14,
    GenericEnd = 15,
    ArgumentsStart = 16,
    ArgumentName = 17,
    ArgumentType = 18,
    ArgumentEnd = 19,
    ArgumentsEnd = 20,
    ReturnType = 21,
    CodeStart = 22,
    StructStart = 23,
    TraitStart = 24,
    ImplStart = 25,
    FunctionStart = 26,
    StructTopElement = 27,
    StructEnd = 28,
    FieldName = 29,
    FieldType = 30,
    FieldValue = 31,
    FieldEnd = 32
}