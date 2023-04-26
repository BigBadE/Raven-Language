use std::convert::Infallible;
use std::ops::{ControlFlow, FromResidual, Try};
use syntax::ParsingError;

#[derive(Clone, Debug)]
pub struct Token {
    pub token_type: TokenTypes,
    pub start: (u32, u32),
    pub start_offset: usize,
    pub end: (u32, u32),
    pub end_offset: usize
}

impl Token {
    pub fn new(token_type: TokenTypes, start: (u32, u32), start_offset: usize, end: (u32, u32), end_offset: usize) -> Self {
        return Self {
            token_type,
            start,
            start_offset,
            end,
            end_offset
        }
    }

    pub fn make_error(&self, file: String, error: String) -> ParsingError {
        return ParsingError::new(file, self.start, self.start_offset, self.end, self.end_offset, error);
    }

    pub fn to_string(&self, buffer: &[u8]) -> String {
        let mut start = self.start_offset;
        let mut end = self.end_offset-1;
        while buffer[start] == b' ' || buffer[start] == b'\t' || buffer[start] == b'\r' || buffer[start] == b'\n' &&
            start < end {
            start += 1;
        }
        while buffer[end] == b' ' || buffer[end] == b'\t' || buffer[end] == b'\r' || buffer[end] == b'\n' &&
            start < end {
            end -= 1;
        }
        return String::from_utf8_lossy(&buffer[start..end+1]).to_string();
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
    FieldEnd = 32,
    LineEnd = 33,
    Operator = 34,
    CodeEnd = 35,
    Variable = 36,
    Integer = 37,
    Float = 38,
    CallingType = 39,
    Return = 40,
    Break = 41,
    Switch = 42,
    For = 43,
    While = 44,
    Else = 45,
    If = 46,
    ParenOpen = 47,
    ParenClose = 48,
    BlockStart = 49,
    BlockEnd = 50,
    New = 51,
    Colon = 52,
    In = 53,
    ImportEnd = 54,
    ReturnTypeArrow = 55,
    ArgumentTypeSeparator = 56,
    ArgumentSeparator = 57,
    Let = 58,
    Equals = 59,
    AttributeEnd = 60
}