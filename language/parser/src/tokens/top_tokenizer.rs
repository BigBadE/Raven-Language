use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::util::{parse_ident, parse_modifier};

pub fn next_top_token(tokenizer: &mut Tokenizer) -> Token {
    return match tokenizer.last.token_type {
        TokenTypes::ImportStart => parse_ident(tokenizer, TokenTypes::Identifier, &[b';']),
        TokenTypes::AttributesStart | TokenTypes::Attribute => if tokenizer.matches("#[") {
            parse_ident(tokenizer, TokenTypes::Attribute, &[b']'])
        } else {
            tokenizer.make_token(TokenTypes::ModifiersStart)
        }
        TokenTypes::ModifiersStart | TokenTypes::Modifier => if let Some(modifier) = parse_modifier(tokenizer) {
            modifier
        } else {
            tokenizer.make_token(TokenTypes::ElemStart)
        }
        TokenTypes::ElemStart => if tokenizer.matches("fn") {
            tokenizer.state = TokenizerState::Function;
            parse_ident(tokenizer, TokenTypes::Identifier, &[b'<', b'('])
        } else if tokenizer.matches("impl") || tokenizer.matches("struct") {
            tokenizer.state = TokenizerState::Structure;
            parse_ident(tokenizer, TokenTypes::Identifier, &[b'<', b'('])
        } else {
            tokenizer.handle_invalid()
        }
        _ => {
            if tokenizer.matches("import") {
                tokenizer.make_token(TokenTypes::ImportStart)
            } else {
                tokenizer.make_token(TokenTypes::AttributesStart)
            }
        }
    }
}

pub fn next_func_token(tokenizer: &mut Tokenizer) -> Token {
    return match tokenizer.last.token_type {
        TokenTypes::Identifier => if tokenizer.matches("<") {
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.matches("(") {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.state = TokenizerState::TopElement;
            tokenizer.handle_invalid()
        }
        TokenTypes::GenericsStart =>
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>']),
        TokenTypes::Generic => if tokenizer.matches(":") {
            tokenizer.make_token(TokenTypes::GenericBound)
        } else if tokenizer.matches(",") {
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>'])
        } else if tokenizer.matches(">") {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::ArgumentsStart | TokenTypes::ArgumentEnd => if tokenizer.matches(")") {
            tokenizer.make_token(TokenTypes::ArgumentsEnd)
        } else {
            parse_ident(tokenizer, TokenTypes::ArgumentName, &[b':', b','])
        },
        TokenTypes::ArgumentName => if tokenizer.matches(":") {
            tokenizer.make_token(TokenTypes::ArgumentType)
        } else {
            //Skip the comma if there is one
            tokenizer.matches(",");
            tokenizer.make_token(TokenTypes::ArgumentEnd)
        },
        TokenTypes::ArgumentsEnd => if tokenizer.matches("->") {
            tokenizer.make_token(TokenTypes::ReturnTypeStart)
        } else if tokenizer.matches("{"){
            tokenizer.state = TokenizerState::Code;
            tokenizer.make_token(TokenTypes::CodeStart)
        } else {
            tokenizer.handle_invalid()
        }
        _ => {
            panic!("How'd you get here?");
        }
    }
}


pub fn next_func_token(tokenizer: &mut Tokenizer) -> Token {
    return match tokenizer.last.token_type {
        TokenTypes::Identifier => if tokenizer.matches("<") {
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.matches("(") {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.state.pop();
            tokenizer.handle_invalid()
        }
        TokenTypes::GenericsStart =>
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>']),
        TokenTypes::Generic => if tokenizer.matches(":") {
            tokenizer.make_token(TokenTypes::GenericBound)
        } else if tokenizer.matches(",") {
            parse_ident(tokenizer, TokenTypes::Generic, &[b':', b',', b'>'])
        } else if tokenizer.matches(">") {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::ArgumentsStart | TokenTypes::ArgumentEnd => if tokenizer.matches(")") {
            tokenizer.make_token(TokenTypes::ArgumentsEnd)
        } else {
            parse_ident(tokenizer, TokenTypes::ArgumentName, &[b':', b','])
        },
        TokenTypes::ArgumentName => if tokenizer.matches(":") {
            tokenizer.make_token(TokenTypes::ArgumentType)
        } else {
            //Skip the comma if there is one
            tokenizer.matches(",");
            tokenizer.make_token(TokenTypes::ArgumentEnd)
        },
        TokenTypes::ArgumentsEnd => if tokenizer.matches("->") {
            tokenizer.make_token(TokenTypes::ReturnTypeStart)
        } else if tokenizer.matches("{") {
            tokenizer.state.push(TokenizerState::Code);
            tokenizer.make_token(TokenTypes::CodeStart)
        } else {
            tokenizer.handle_invalid()
        }
        _ => {
            panic!("How'd you get here?");
        }
    }
}
