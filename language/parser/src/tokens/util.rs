use syntax::MODIFIERS;
use crate::tokens::tokenizer::Tokenizer;
use crate::tokens::tokens::{Token, TokenTypes};

pub fn parse_ident(tokenizer: &mut Tokenizer, token_type: TokenTypes, end: &[u8]) -> Token {
    while !end.contains(&tokenizer.next_included()?) {}
    return Token::new(token_type, tokenizer.last.end, tokenizer.index-1);
}

pub fn parse_modifier(tokenizer: &mut Tokenizer) -> Option<Token> {
    for modifier in MODIFIERS {
        if tokenizer.matches(format!("{}", modifier).as_str()) {
            return Some(Token::new(TokenTypes::Modifier, tokenizer.last.end, tokenizer.index-1));
        }
    }
    return None;
}