use crate::tokens::tokenizer::Tokenizer;
use crate::tokens::tokens::{Token, TokenTypes};

pub fn next_code_token(tokenizer: &mut Tokenizer) -> Token {
    return tokenizer.make_token(TokenTypes::EOF);
}