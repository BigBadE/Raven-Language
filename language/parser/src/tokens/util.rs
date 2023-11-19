use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};
use syntax::MODIFIERS;

/// Parses to one of the provided end characters
pub fn parse_to_character(tokenizer: &mut Tokenizer, token_type: TokenTypes, end: &[u8]) -> Token {
    while !end.contains(&tokenizer.next_included()?) {}
    tokenizer.index -= 1;
    return tokenizer.make_token(token_type);
}

/// Parses the value of an attribute
pub fn parse_attribute_val(tokenizer: &mut Tokenizer, token_type: TokenTypes) -> Token {
    let mut depth = 1;
    let mut last;
    loop {
        last = tokenizer.next_included()?;
        if last == b']' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if last == b'[' {
            depth += 1;
        }
    }
    tokenizer.index -= 1;
    return tokenizer.make_token(token_type);
}

/// Parses until a non-acceptable token for a variable
pub fn parse_acceptable(tokenizer: &mut Tokenizer, token_type: TokenTypes) -> Token {
    loop {
        if tokenizer.index == tokenizer.len {
            return tokenizer.make_token(TokenTypes::EOF);
        }
        let character = tokenizer.buffer[tokenizer.index] as char;
        if !character.is_alphanumeric() && character != ':' && character != '_' {
            if tokenizer.buffer[tokenizer.index - 1] == b':' {
                tokenizer.index -= 1;
            }
            return tokenizer.make_token(token_type);
        }
        tokenizer.index += 1;
    }
}

/// Parses numbers
pub fn parse_numbers(tokenizer: &mut Tokenizer) -> Token {
    let mut float = false;

    loop {
        if tokenizer.index == tokenizer.len {
            return tokenizer.make_token(TokenTypes::EOF);
        }
        let character = tokenizer.buffer[tokenizer.index] as char;
        if character == '.' {
            if float {
                // If there's two periods in a row it's not a float, return the integer.
                return if tokenizer.buffer[tokenizer.index - 1] == b'.' {
                    tokenizer.index -= 1;
                    tokenizer.make_token(TokenTypes::Integer)
                } else {
                    tokenizer.make_token(TokenTypes::Float)
                };
            } else {
                float = true;
            }
        } else {
            if !character.is_numeric() {
                return if float {
                    // If no number is after the period assume it's a method call not a float.
                    if tokenizer.buffer[tokenizer.index - 1] == b'.' {
                        tokenizer.index -= 1;
                        tokenizer.make_token(TokenTypes::Integer)
                    } else {
                        tokenizer.make_token(TokenTypes::Float)
                    }
                } else {
                    tokenizer.make_token(TokenTypes::Integer)
                };
            }
        }
        tokenizer.index += 1;
    }
}

/// Parses any modifiers.
pub fn parse_modifier(tokenizer: &mut Tokenizer) -> Option<Token> {
    for modifier in MODIFIERS {
        if tokenizer.matches(format!("{}", modifier).as_str()) {
            return Some(tokenizer.make_token(TokenTypes::Modifier));
        }
    }
    return None;
}

/// Parses a string.
pub fn parse_string(tokenizer: &mut Tokenizer) -> Token {
    loop {
        if tokenizer.len == tokenizer.index {
            return tokenizer.make_token(TokenTypes::EOF);
        }

        // get the next character
        let next = tokenizer.buffer[tokenizer.index];
        tokenizer.index += 1;

        match next {
            // if the last character was a \, then the quote is escaped, so don't end the string here
            b'"' => {
                return if
                /*tokenizer.last.token_type != TokenTypes::StringEscape*/
                tokenizer.buffer[tokenizer.index - 1] != b'\\' {
                    tokenizer.state =
                        if tokenizer.state == TokenizerState::STRING_TO_CODE_STRUCT_TOP {
                            TokenizerState::CODE_TO_STRUCT_TOP
                        } else {
                            TokenizerState::CODE
                        };
                    tokenizer.make_token(TokenTypes::StringEnd)
                } else {
                    tokenizer.make_token(TokenTypes::StringStart)
                };
            }
            b'\\' => {
                // if it is a hex value, then increment the tokenizer by an extra 2 because
                // the escape character is 4 characters long instead of 2 (ex. \xAA)
                if tokenizer.buffer[tokenizer.index] == b'x' {
                    tokenizer.index += 2;
                }

                // increment the tokenizer so that it includes the \
                // if you didn't do this, then the character being escaped (ex. n or t or r)
                //   would be included in the string
                tokenizer.index += 1;

                return tokenizer.make_token(TokenTypes::StringEscape);
            }
            _ => {}
        }
    }
}

/// Parses a generic type, only for generics in a function/impl and not for types which currently are tokenized in the parser.
pub fn next_generic(tokenizer: &mut Tokenizer) -> Token {
    return match &tokenizer.last.token_type {
        TokenTypes::GenericsStart | TokenTypes::GenericEnd => {
            parse_to_character(tokenizer, TokenTypes::Generic, &[b':', b',', b'>', b'<'])
        }
        //              T       : Test       <             Other   <             Second  >               >               ,          E       : Yep
        //GenericsStart Generic GenericBound GenericsStart Generic GenericsStart Generic GenericBoundEnd GenericBoundEnd GenericEnd Generic GenericBound
        TokenTypes::Generic | TokenTypes::GenericBound | TokenTypes::GenericBoundEnd => {
            if tokenizer.matches(":") || tokenizer.matches("+") {
                parse_to_character(
                    tokenizer,
                    TokenTypes::GenericBound,
                    &[b',', b'+', b'>', b'<'],
                )
            } else if tokenizer.matches("<") {
                tokenizer.generic_depth += 1;
                tokenizer.make_token(TokenTypes::GenericsStart)
            } else if tokenizer.matches(",") {
                tokenizer.make_token(TokenTypes::GenericEnd)
            } else if tokenizer.matches(">") {
                tokenizer.generic_depth -= 1;
                if tokenizer.generic_depth == 0 {
                    // The generics are done, break of out the generic state
                    tokenizer.state = match tokenizer.state {
                        TokenizerState::GENERIC_TO_FUNC => TokenizerState::FUNCTION,
                        TokenizerState::GENERIC_TO_FUNC_TO_STRUCT_TOP => {
                            TokenizerState::FUNCTION_TO_STRUCT_TOP
                        }
                        TokenizerState::GENERIC_TO_STRUCT => TokenizerState::STRUCTURE,
                        TokenizerState::GENERIC_TO_IMPL => TokenizerState::IMPLEMENTATION,
                        _ => panic!("Unexpected generic state!"),
                    };
                    // Reset the generic depth variable in the tokenizer
                    tokenizer.generic_depth = 1;
                    tokenizer.make_token(TokenTypes::GenericsEnd)
                } else {
                    tokenizer.make_token(TokenTypes::GenericBoundEnd)
                }
            } else {
                tokenizer.handle_invalid()
            }
        }
        token_type => panic!("How'd you get here? {:?}", token_type),
    };
}
