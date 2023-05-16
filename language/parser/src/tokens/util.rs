use syntax::MODIFIERS;
use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};

pub fn parse_ident(tokenizer: &mut Tokenizer, token_type: TokenTypes, end: &[u8]) -> Token {
    while !end.contains(&tokenizer.next_included()?) {}
    tokenizer.index -= 1;
    return tokenizer.make_token(token_type);
}

pub fn parse_acceptable(tokenizer: &mut Tokenizer, token_type: TokenTypes) -> Token {
    loop {
        if tokenizer.index == tokenizer.len {
            return tokenizer.make_token(TokenTypes::EOF);
        }
        let character = tokenizer.buffer[tokenizer.index] as char;
        if !character.is_alphabetic() && character != ':' && character != '_' {
            if tokenizer.buffer[tokenizer.index-1] == b':' {
                tokenizer.index -= 1;
            }
            return tokenizer.make_token(token_type);
        }
        tokenizer.index += 1;
    }
}

pub fn parse_numbers(tokenizer: &mut Tokenizer) -> Token {
    let mut float = false;

    loop {
        if tokenizer.index == tokenizer.len {
            return tokenizer.make_token(TokenTypes::EOF);
        }
        let character = tokenizer.buffer[tokenizer.index] as char;
        if character == '.' {
            if float {
                return tokenizer.make_token(TokenTypes::Float);
            } else {
                float = true;
            }
        } else {
            if !character.is_numeric() {
                return if float {
                    tokenizer.make_token(TokenTypes::Float)
                } else {
                    tokenizer.make_token(TokenTypes::Integer)
                }
            }
        }
        tokenizer.index += 1;
    }
}

pub fn parse_modifier(tokenizer: &mut Tokenizer) -> Option<Token> {
    for modifier in MODIFIERS {
        if tokenizer.matches(format!("{}", modifier).as_str()) {
            return Some(tokenizer.make_token(TokenTypes::Modifier));
        }
    }
    return None;
}

pub fn next_string(tokenizer: &mut Tokenizer) -> Token {
    loop {
        if tokenizer.next_included()? == b'"' && tokenizer.last.token_type != TokenTypes::StringEscape {
            if tokenizer.state == TokenizerState::STRING_TO_CODE_STRUCT_TOP {
                tokenizer.state = TokenizerState::CODE_TO_STRUCT_TOP + (tokenizer.state ^ 0xFF);
            } else {
                tokenizer.state = TokenizerState::CODE + (tokenizer.state ^ 0xFF);
            }
            return tokenizer.make_token(TokenTypes::StringEnd);
        }
    }
}

pub fn next_generic(tokenizer: &mut Tokenizer) -> Token {
    return match &tokenizer.last.token_type {
        TokenTypes::GenericsStart =>
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>']),
        TokenTypes::Generic | TokenTypes::GenericBound => if tokenizer.matches(":") || tokenizer.matches("+") {
            parse_ident(tokenizer, TokenTypes::GenericBound, &[b',', b'+', b'>'])
        } else if tokenizer.matches(",") {
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>'])
        } else if tokenizer.matches(">") {
            tokenizer.state = match tokenizer.state {
                TokenizerState::GENERIC_TO_FUNC => TokenizerState::FUNCTION,
                TokenizerState::GENERIC_TO_FUNC_TOP => TokenizerState::FUNCTION_TO_STRUCT_TOP,
                TokenizerState::GENERIC_TO_STRUCT => TokenizerState::STRUCTURE,
                TokenizerState::GENERIC_TO_IMPL => TokenizerState::IMPLEMENTATION,
                _ => panic!("Unexpected generic state!")
            };
            tokenizer.make_token(TokenTypes::GenericEnd)
        } else {
            tokenizer.handle_invalid()
        },
        token_type => panic!("How'd you get here? {:?}", token_type)
    }
}

#[cfg(test)]
pub fn check_types(types: &[TokenTypes], testing: &str, state: u64) {
    let mut tokenizer = Tokenizer::new(testing.as_bytes());
    tokenizer.state = state;
    let mut tokens: Vec<Token> = Vec::new();
    loop {
        let token = tokenizer.next();
        match token.token_type.clone() {
            TokenTypes::InvalidCharacters => assert!(false, "Failed at state {:?} (last: {:?}):\n{}\n from {:?}", tokenizer.state, tokens.last().unwrap().token_type,
                                                     String::from_utf8_lossy(&tokenizer.buffer[tokens.last().unwrap().end.1 as usize..tokenizer.index as usize]),
                                                     &tokens[tokens.len() - 5.min(tokens.len())..tokens.len() - 1]),
            TokenTypes::CodeStart | TokenTypes::EOF => {
                tokens.push(token);
                break;
            }
            _ => tokens.push(token)
        }
    }
    for i in 0..types.len().max(tokens.len()) {
        if i > types.len() {
            assert!(false, "Hit end of types!");
        } else if i > tokens.len() {
            assert!(false, "Hit end of tokens!");
        }
        println!("{:?} vs {:?} (\"{}\")", types[i], tokens.get(i).as_ref().unwrap().token_type,
                 String::from_utf8_lossy(&tokenizer.buffer[tokens.get(i).as_ref().unwrap().start.1 as usize..
                     tokens.get(i).as_ref().unwrap().end.1 as usize]));
        assert_eq!(types[i], tokens.get(i).as_ref().unwrap().token_type);
    }
}