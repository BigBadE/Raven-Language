use crate::ParsingError;
use std::convert::Infallible;
use std::ops::{ControlFlow, FromResidual, Try};

/// A token is a single string of characters in the file.
/// For example, keywords, variables, etc... are a single token.
#[derive(Clone, Debug)]
pub struct Token {
    /// The type of the token
    pub token_type: TokenTypes,
    /// The starting line and index in that line of the token.
    pub start: (u32, u32),
    /// The offset to the start of the token
    pub start_offset: usize,
    /// The ending line and index in that line of the token.
    pub end: (u32, u32),
    /// The offset to the end of the token
    pub end_offset: usize,
    /// Data about the code block around this token
    pub code_data: Option<TokenCodeData>,
}

impl Token {
    /// Creates a new token, usually done by the tokenizer
    pub fn new(
        token_type: TokenTypes,
        code_data: Option<TokenCodeData>,
        start: (u32, u32),
        start_offset: usize,
        end: (u32, u32),
        end_offset: usize,
    ) -> Self {
        return Self { token_type, start, start_offset, end, end_offset, code_data };
    }

    /// Creates an error for this part of the file.
    pub fn make_error(&self, file: String, error: String) -> ParsingError {
        return ParsingError::new(file, self.start, self.start_offset, self.end, self.end_offset, error);
    }

    /// Turns the token into the string it points to.
    pub fn to_string(&self, buffer: &[u8]) -> String {
        let mut start = self.start_offset;
        let mut end = self.end_offset - 1;
        while buffer[start] == b' '
            || buffer[start] == b'\t'
            || buffer[start] == b'\r'
            || buffer[start] == b'\n' && start < end
        {
            start += 1;
        }
        while buffer[end] == b' ' || buffer[end] == b'\t' || buffer[end] == b'\r' || buffer[end] == b'\n' && start < end {
            end -= 1;
        }
        return String::from_utf8_lossy(&buffer[start..=end]).to_string();
    }
}

/// This allows for Tokens to be used in the Result type.
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

/// Required for Try
impl FromResidual<Token> for Token {
    fn from_residual(residual: Token) -> Self {
        return residual;
    }
}

/// Required for Try
impl FromResidual<Result<Infallible, Token>> for Token {
    fn from_residual(residual: Result<Infallible, Token>) -> Token {
        return residual.unwrap_err();
    }
}

#[derive(Clone, Debug)]
pub struct CodeErrorToken {
    pub token: Token,
    pub file_name: String,
}

impl CodeErrorToken {
    pub fn new(token: Token, file_name: String) -> Self {
        return Self { token, file_name };
    }

    pub fn make_error(&self, message: String) -> ParsingError {
        return self.token.make_error(self.file_name.clone(), message);
    }

    pub fn change_token_end(&mut self, end_token: &Token) {
        self.token.end = end_token.end;
        self.token.end_offset = end_token.end_offset;
    }

    pub fn make_empty() -> Self {
        return Self { token: Token::new(TokenTypes::Colon, None, (0, 0), 0, (0, 0), 0), file_name: String::default() };
    }
}

/// Data about the current code block
#[derive(Clone, Debug)]
pub struct TokenCodeData {
    /// Start line of the code block
    pub start_line: u32,
    /// End line of the code block
    pub end_line: u32,
}

/// The different types of tokens.
/// The numerical value assigned is arbitrary and used
/// for deriving functions like equals
/// and some IDEs require a numerical id for each token.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenTypes {
    /// The start of a file
    Start = 0,
    /// The end of a file
    EOF = 1,
    /// Invalid characters that must be error handled
    InvalidCharacters = 2,
    /// The start of a string
    StringStart = 3,
    /// A string escape character ("/") which must be handled specially
    StringEscape = 4,
    /// The end of a string
    StringEnd = 5,
    /// The import keyword
    ImportStart = 6,
    /// A non-keyword identifier, like a variable/function/import name
    Identifier = 7,
    /// Start of attributes, can be empty
    AttributesStart = 8,
    /// A single attribute
    Attribute = 9,
    /// Start of modifiers, can be empty
    ModifiersStart = 10,
    /// A single modifier
    Modifier = 11,
    /// Start of generics ("<")
    GenericsStart = 12,
    /// A single generic
    Generic = 13,
    /// The bound symbol of the generic (":")
    GenericBound = 14,
    /// The end of a single generic
    GenericEnd = 15,
    /// The start of a function's arguments
    ArgumentsStart = 16,
    /// The name of the argument
    ArgumentName = 17,
    /// The argument's type
    ArgumentType = 18,
    /// The end of a single argument
    ArgumentsEnd = 20,
    /// The end of a function's arguments
    ArgumentEnd = 19,
    /// A function's return type
    ReturnType = 21,
    /// The start of code ("{")
    CodeStart = 22,
    /// The start of a struct ("struct")
    StructStart = 23,
    /// The start of a trait ("trait")
    TraitStart = 24,
    /// The start of an impl ("impl")
    ImplStart = 25,
    /// The start of a function ("fn")
    FunctionStart = 26,
    /// A top element inside a struct
    StructTopElement = 27,
    /// The end of the struct
    StructEnd = 28,
    /// The name of a field
    FieldName = 29,
    /// The type of a field
    FieldType = 30,
    /// A field's value, which is followed by code
    FieldValue = 31,
    /// The end of the field
    FieldEnd = 32,
    /// The end of a line of code (";")
    LineEnd = 33,
    /// An operator, any non-illegal character in code like +, -, etc... which is checked later
    Operator = 34,
    /// The end of a block of code ("}")
    CodeEnd = 35,
    /// A variable name
    Variable = 36,
    /// An integer
    Integer = 37,
    /// A float
    Float = 38,
    /// A type being called, always comes after a period (like variable.method())
    ///                                                                 ^^^^^^
    CallingType = 39,
    /// The return keyword
    Return = 40,
    /// The break keyword
    Break = 41,
    /// The switch keyword
    Switch = 42,
    /// The for keyword
    For = 43,
    /// The while keyword
    While = 44,
    /// The else keyword
    Else = 45,
    /// The if keyword
    If = 46,
    /// An opening parenthesis
    ParenOpen = 47,
    /// A closing parenthesis
    ParenClose = 48,
    /// The start of a block of a code ("{")
    /// Cannot be the top-level { (which is CodeStart)
    BlockStart = 49,
    /// The end of a block of a code ("}")
    /// Cannot be the top-level } (which is CodeEnd)
    BlockEnd = 50,
    /// The new keyword
    New = 51,
    /// A colon
    Colon = 52,
    /// The in keyword
    In = 53,
    /// The end of an import (";")
    ImportEnd = 54,
    /// The return type arrow in a function header ("->")
    ReturnTypeArrow = 55,
    /// The seperator between an argument and its type (":")
    ArgumentTypeSeparator = 56,
    /// The seperate between two arguments (",")
    ArgumentSeparator = 57,
    /// The let keyword
    Let = 58,
    /// The equals sign
    Equals = 59,
    /// The end of a single attribute
    AttributeEnd = 60,
    /// The seperator between a field and its value (":")
    FieldSeparator = 61,
    /// The period symbol
    Period = 62,
    /// A comment, started by "//" and spanning one line or started by "/*" and ended by "*/"
    Comment = 63,
    /// The true keyword
    True = 64,
    /// The false keyword
    False = 65,
    /// The start of a single attribute
    AttributeStart = 66,
    /// The end of a generic bound (">")
    GenericBoundEnd = 67,
    /// The end of a group of generics (">")
    GenericsEnd = 68,
    /// The do keyword
    Do = 69,
    /// A single character surrounded by single quotes ('a')
    Char = 70,
    /// A blank line
    BlankLine = 71,
}
