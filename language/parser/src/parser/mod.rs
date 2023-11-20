/// This package turns a list of tokens from the tokenizer into a Syntax. See lib::parse and Syntax

/// The code parser
pub mod code_parser;
/// Parser for control statements like if, for, while, etc...
pub mod control_parser;
/// Parser for functions
pub mod function_parser;
/// Parser for operators
pub mod operator_parser;
/// Parser for structs
pub mod struct_parser;
/// Parser for top elements like imports and struct/func headers
pub mod top_parser;
/// Utility parsing functions
pub mod util;
