/// This folder contains the tokenizer (also known as a Lexer)
/// Explainer article: https://en.wikipedia.org/wiki/Lexical_analysis

/// Tokenizes code
pub mod code_tokenizer;
/// The base tokenizer types used in all tokenizers
pub mod tokenizer;
/// Tokenizer for top elements like structs, functions, and impls
pub mod top_tokenizer;
/// Utility functions used in other files
pub mod util;
