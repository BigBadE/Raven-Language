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
        } else if tokenizer.matches("fn") {
            if tokenizer.state == TokenizerState::TopElementToStruct {
                tokenizer.state = TokenizerState::FunctionToStructTop;
            } else {
                tokenizer.state = TokenizerState::Function;
            }
            parse_ident(tokenizer, TokenTypes::Identifier, &[b'<', b'('])
        } else if tokenizer.matches("struct") || tokenizer.matches("trait") {
            if tokenizer.state == TokenizerState::TopElementToStruct {
                tokenizer.handle_invalid()
            } else {
                tokenizer.state = TokenizerState::Structure;
                parse_ident(tokenizer, TokenTypes::Identifier, &[b'<', b'('])
            }
        } else if tokenizer.matches("impl") {
            if tokenizer.state == TokenizerState::TopElementToStruct {
                tokenizer.handle_invalid()
            } else {
                tokenizer.state = TokenizerState::GenericToStruct;
                tokenizer.make_token(TokenTypes::ImplStart)
            }
        } else {
            parse_ident(tokenizer, TokenTypes::FieldName, &[b':', b'='])
        },
        TokenTypes::FieldName => if tokenizer.last() == b':' {
            tokenizer.make_token(TokenTypes::FieldType)
        } else if tokenizer.last() == b';' {
            next_top_token(tokenizer)
        } else {
            tokenizer.handle_invalid()
        }
        TokenTypes::FieldType => if tokenizer.last() == b'=' {
            if tokenizer.state == TokenizerState::TopElementToStruct {
                tokenizer.state = TokenizerState::CodeToStructTop;
            } else {
                tokenizer.state = TokenizerState::Code;
            }
            tokenizer.make_token(TokenTypes::FieldValue)
        } else {
            next_top_token(tokenizer)
        }
        _ => {
            if tokenizer.matches("import") {
                tokenizer.make_token(TokenTypes::ImportStart)
            } else if tokenizer.matches("}") && tokenizer.state == TokenizerState::TopElementToStruct {
                tokenizer.state = TokenizerState::TopElement;
                tokenizer.make_token(TokenTypes::StructEnd)
            } else {
                tokenizer.make_token(TokenTypes::AttributesStart)
            }
        }
    };
}

pub fn next_func_token(tokenizer: &mut Tokenizer) -> Token {
    return match &tokenizer.last.token_type {
        TokenTypes::FunctionStart => parse_ident(tokenizer, TokenTypes::Identifier, &[b'<', b'(']),
        TokenTypes::Identifier => if tokenizer.last() == b'<' {
            tokenizer.state = TokenizerState::GenericToFunc;
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.last() == b'(' {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.state = TokenizerState::TopElement;
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
            tokenizer.state = TokenizerState::Code;
            tokenizer.make_token(TokenTypes::CodeStart)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::ReturnType => if tokenizer.last() == b'{' {
            tokenizer.state = TokenizerState::Code;
            tokenizer.make_token(TokenTypes::CodeStart)
        } else if tokenizer.last() == b';' {
            if tokenizer.state == TokenizerState::Function {
                tokenizer.state = TokenizerState::TopElement;
            } else if tokenizer.state == TokenizerState::FunctionToStructTop {
                tokenizer.state = TokenizerState::TopElementToStruct;
            }
            next_top_token(tokenizer)
        } else {
            tokenizer.handle_invalid()
        }
        token => {
            panic!("How'd you get here? {:?}", token);
        }
    };
}


pub fn next_struct_token(tokenizer: &mut Tokenizer) -> Token {
    match tokenizer.last.token_type {
        TokenTypes::StructStart | TokenTypes::TraitStart => parse_ident(tokenizer, TokenTypes::Identifier, &[b'{']),
        TokenTypes::Identifier => if tokenizer.last() == b'{' {
            tokenizer.state = TokenizerState::TopElementToStruct;
            tokenizer.make_token(TokenTypes::StructTopElement)
        } else {
            tokenizer.handle_invalid()
        },
        _ => panic!("How'd you get here?")
    }
}

pub fn next_implementation_token(tokenizer: &mut Tokenizer) -> Token {
    match &tokenizer.last.token_type {
        TokenTypes::ImplStart => if tokenizer.matches("<") {
            tokenizer.state = TokenizerState::GenericToImpl;
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else {
            tokenizer.parse_to_next_space(TokenTypes::Identifier)
        }
        TokenTypes::GenericEnd => if tokenizer.matches("for") {
            tokenizer.state = TokenizerState::Structure;
            tokenizer.make_token(TokenTypes::TraitStart)
        } else {
            tokenizer.parse_to_first(TokenTypes::Identifier, b'<', b' ')
        }
        TokenTypes::Identifier => if tokenizer.last() == b'<' {
            tokenizer.state = TokenizerState::GenericToImpl;
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.matches("for") {
            tokenizer.state = TokenizerState::Structure;
            tokenizer.make_token(TokenTypes::TraitStart)
        } else {
            tokenizer.handle_invalid()
        },
        token => panic!("How'd you get here? {:?}", token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eof() {
        let mut types = Vec::new();
        add_header(0, &mut types);
        types.push(TokenTypes::EOF);
        let testing = "";
        check_types(&types, testing);
    }

    #[test]
    fn test_func() {
        let mut types = Vec::new();
        add_header(2, &mut types);
        types.push(TokenTypes::Identifier);
        add_generics(1, true, &mut types);
        add_arguments(2, true, &mut types);
        types.push(TokenTypes::ReturnType);
        types.push(TokenTypes::CodeStart);
        let testing = "pub internal fn testing<T: Bound>(self, arg2: TypeAgain) -> ReturnType {}";
        check_types(&types, testing);
    }

    #[test]
    fn test_struct() {
        let mut types = Vec::new();
        add_header(1, &mut types);
        types.push(TokenTypes::TraitStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, true, &mut types);
        add_header(0, &mut types);
        types.push(TokenTypes::FunctionStart);
        types.push(TokenTypes::Identifier);
        add_arguments(0, false, &mut types);
        types.push(TokenTypes::StructEnd);
        add_header(1, &mut types);
        types.push(TokenTypes::StructStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, true, &mut types);
        add_header(1, &mut types);
        types.push(TokenTypes::FieldName);
        types.push(TokenTypes::FieldType);
        types.push(TokenTypes::StructEnd);
        add_header(0, &mut types);
        types.push(TokenTypes::ImplStart);
        add_generics(2, true, &mut types);
        types.push(TokenTypes::Identifier);
        add_generics(1, false, &mut types);
        types.push(TokenTypes::StructStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, false, &mut types);
        add_header(1, &mut types);
        types.push(TokenTypes::FunctionStart);
        add_arguments(0, false, &mut types);
        types.push(TokenTypes::CodeStart);

        let testing = "pub trait Testing<T: Bound> {\
            pub fn trait_func();\
        }\
        pub struct TestStruct<T: OtherBound> {\
            pub field: MyField\
        }\
        impl<T: Bound, E: OtherBound> Test<T> for TestStruct<E> {\
            pub fn trait_func() {}\
        }";
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
                    break;
                }
                _ => tokens.push(token)
            }
        }
        assert_eq!(types.len(), tokens.len());
        for i in 0..types.len() {
            assert_eq!(types[i], tokens.get(i).as_ref().unwrap().token_type);
        }
    }

    fn add_header(modifiers: u8, input: &mut Vec<TokenTypes>) {
        input.push(TokenTypes::AttributesStart);
        input.push(TokenTypes::ModifiersStart);
        for _ in 0..modifiers {
            input.push(TokenTypes::Modifier);
        }
    }

    fn add_generics(generics: u8, bound: bool, input: &mut Vec<TokenTypes>) {
        input.push(TokenTypes::GenericsStart);
        for _ in 0..generics {
            input.push(TokenTypes::Generic);
            if bound {
                input.push(TokenTypes::GenericBound);
            }
        }
        input.push(TokenTypes::GenericEnd);
    }

    fn add_arguments(arguments: u8, first_is_self: bool, input: &mut Vec<TokenTypes>) {
        input.push(TokenTypes::ArgumentsStart);
        if arguments > 0 && first_is_self {
            input.push(TokenTypes::ArgumentName);
            input.push(TokenTypes::ArgumentEnd);
        }

        for _ in (0 + first_is_self as u8)..arguments {
            input.push(TokenTypes::ArgumentName);
            input.push(TokenTypes::ArgumentType);
            input.push(TokenTypes::ArgumentEnd);
        }
        input.push(TokenTypes::ArgumentsEnd);
    }
}