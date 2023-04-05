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


pub fn next_string(tokenizer: &mut Tokenizer) -> Token {
    loop {
        if tokenizer.next_included()? == b'"' && tokenizer.last.token_type != TokenTypes::StringEscape {
            tokenizer.state.pop();
            return tokenizer.make_token(TokenTypes::StringEnd);
        }
    }
}

pub fn next_generic(tokenizer: &mut Tokenizer) -> Token {
    return match tokenizer.last.token_type {
        TokenTypes::GenericsStart =>
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>']),
        TokenTypes::Generic => if tokenizer.last() == b':' {
            parse_ident(tokenizer, TokenTypes::GenericBound, &[b',', b'+', b'>'])
        } else if tokenizer.last() == b',' {
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>'])
        } else if tokenizer.last() == b'>' {
            tokenizer.state.pop();
            tokenizer.make_token(TokenTypes::GenericEnd)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::GenericBound => if tokenizer.last() == b'+' {
            parse_ident(tokenizer, TokenTypes::GenericBound, &[b',', b'+', b'>'])
        } else if tokenizer.last() == b',' {
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>'])
        } else if tokenizer.last() == b'>' {
            tokenizer.state.pop();
            tokenizer.make_token(TokenTypes::GenericEnd)
        } else {
            tokenizer.handle_invalid()
        },
        _ => panic!("How'd you get here?")
    }
}