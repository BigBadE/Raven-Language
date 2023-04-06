use syntax::MODIFIERS;
use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
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
            if tokenizer.state == TokenizerState::StringToCodeStructTop {
                tokenizer.state = TokenizerState::CodeToStructTop;
            } else {
                tokenizer.state = TokenizerState::Code;
            }
            return tokenizer.make_token(TokenTypes::StringEnd);
        }
    }
}

pub fn next_generic(tokenizer: &mut Tokenizer) -> Token {
    return match &tokenizer.last.token_type {
        TokenTypes::GenericsStart =>
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>']),
        TokenTypes::Generic | TokenTypes::GenericBound => if tokenizer.last() == b':' || tokenizer.last() == b'+' {
            parse_ident(tokenizer, TokenTypes::GenericBound, &[b',', b'+', b'>'])
        } else if tokenizer.last() == b',' {
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>'])
        } else if tokenizer.last() == b'>' {
            if tokenizer.state == TokenizerState::GenericToImpl {
                tokenizer.state = TokenizerState::Implementation;
            } else if tokenizer.state == TokenizerState::GenericToStruct {
                tokenizer.state = TokenizerState::Structure;
            } else {
                tokenizer.state = TokenizerState::Function;
            }
            tokenizer.make_token(TokenTypes::GenericEnd)
        } else {
            tokenizer.handle_invalid()
        },
        token_type => panic!("How'd you get here? {:?}", token_type)
    }
}