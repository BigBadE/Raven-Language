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
            tokenizer.state.push(TokenizerState::Function);
            parse_ident(tokenizer, TokenTypes::Identifier, &[b'<', b'('])
        } else if tokenizer.matches("impl") || tokenizer.matches("struct") {
            tokenizer.state.push(TokenizerState::Structure);
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
    };
}

pub fn next_func_token(tokenizer: &mut Tokenizer) -> Token {
    return match &tokenizer.last.token_type {
        TokenTypes::Identifier => if tokenizer.last() == b'<' {
            tokenizer.state.push(TokenizerState::Generic);
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.last() == b'(' {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.state.pop();
            tokenizer.handle_invalid()
        },
        TokenTypes::GenericEnd => {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        }
        TokenTypes::ArgumentsStart | TokenTypes::ArgumentEnd => if tokenizer.last() == b')' {
            tokenizer.make_token(TokenTypes::ArgumentsEnd)
        } else {
            parse_ident(tokenizer, TokenTypes::ArgumentName, &[b':', b','])
        },
        TokenTypes::ArgumentName => if tokenizer.last() == b':' {
            parse_ident(tokenizer, TokenTypes::ArgumentType, &[b',', b')'])
        } else {
            //Skip the comma if there is one
            tokenizer.matches(",");
            tokenizer.make_token(TokenTypes::ArgumentEnd)
        },
        TokenTypes::ArgumentType => if tokenizer.last() == b',' {
            parse_ident(tokenizer, TokenTypes::ArgumentName, &[b':', b','])
        } else {
            tokenizer.make_token(TokenTypes::ArgumentEnd)
        },
        TokenTypes::ArgumentsEnd => if tokenizer.matches("->") {
            parse_ident(tokenizer, TokenTypes::ReturnType, &[b'{'])
        } else if tokenizer.matches("{") {
            tokenizer.state.push(TokenizerState::Code);
            tokenizer.make_token(TokenTypes::CodeStart)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::ReturnType => if tokenizer.last() == b'{' {
            tokenizer.state.push(TokenizerState::Code);
            tokenizer.make_token(TokenTypes::CodeStart)
        } else {
            tokenizer.handle_invalid()
        }
        token => {
            panic!("How'd you get here? {:?}", token);
        }
    };
}


pub fn next_struct_token(tokenizer: &mut Tokenizer) -> Token {
    panic!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eof() {
        let types = [TokenTypes::AttributesStart, TokenTypes::ModifiersStart, TokenTypes::ElemStart,
            TokenTypes::EOF];
        let testing = "";
        check_types(&types, testing);
    }

    #[test]
    fn test_to_struct() {
        let types = [TokenTypes::AttributesStart, TokenTypes::ModifiersStart, TokenTypes::Modifier, TokenTypes::Modifier, TokenTypes::ElemStart,
            TokenTypes::Identifier, TokenTypes::GenericsStart, TokenTypes::Generic, TokenTypes::GenericBound, TokenTypes::GenericEnd,
            TokenTypes::ArgumentsStart, TokenTypes::ArgumentName, TokenTypes::ArgumentEnd, TokenTypes::ArgumentName, TokenTypes::ArgumentType,
            TokenTypes::ArgumentEnd, TokenTypes::ArgumentsEnd, TokenTypes::ReturnType, TokenTypes::CodeStart];
        let testing = "pub internal fn testing<T: Bound>(self, arg2: TypeAgain) -> ReturnType {}";
        check_types(&types, testing);
    }

    fn check_types(types: &[TokenTypes], testing: &str) {
        let mut tokenizer = Tokenizer::new(testing.as_bytes());
        let mut tokens = Vec::new();
        loop {
            let token = tokenizer.next();
            match token.token_type.clone() {
                TokenTypes::InvalidCharacters => assert!(false),
                TokenTypes::CodeStart | TokenTypes::EOF => {
                    tokens.push(token);
                    break
                },
                _ => tokens.push(token)
            }
        }
        assert_eq!(types.len(), tokens.len());
        for i in 0..types.len() {
            assert_eq!(types[i], tokens.get(i).as_ref().unwrap().token_type);
        }
    }
}