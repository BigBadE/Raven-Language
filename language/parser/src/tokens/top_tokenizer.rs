use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::util::{parse_ident, parse_modifier};

pub fn next_top_token(tokenizer: &mut Tokenizer) -> Token {
    return match tokenizer.last.token_type {
        TokenTypes::ImportStart => parse_ident(tokenizer, TokenTypes::Identifier, &[b';']),
        TokenTypes::Attribute => if tokenizer.matches("]") {
            tokenizer.make_token(TokenTypes::AttributeEnd)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::AttributesStart | TokenTypes::AttributeEnd => if tokenizer.matches("#[") {
            parse_ident(tokenizer, TokenTypes::Attribute, &[b']'])
        } else {
            tokenizer.make_token(TokenTypes::ModifiersStart)
        }
        TokenTypes::ModifiersStart | TokenTypes::Modifier => if let Some(modifier) = parse_modifier(tokenizer) {
            modifier
        } else if tokenizer.matches("fn") {
            if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
                tokenizer.state = TokenizerState::FUNCTION_TO_STRUCT_TOP;
            } else {
                tokenizer.state = TokenizerState::FUNCTION;
            }
            tokenizer.make_token(TokenTypes::FunctionStart)
        } else if tokenizer.matches("struct") {
            if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
                tokenizer.handle_invalid()
            } else {
                tokenizer.state = TokenizerState::STRUCTURE;
                tokenizer.make_token(TokenTypes::StructStart)
            }
        } else if tokenizer.matches("trait") {
            if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
                tokenizer.handle_invalid()
            } else {
                tokenizer.state = TokenizerState::STRUCTURE;
                tokenizer.make_token(TokenTypes::TraitStart)
            }
        } else if tokenizer.matches("impl") {
            if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
                tokenizer.handle_invalid()
            } else {
                tokenizer.state = TokenizerState::IMPLEMENTATION;
                tokenizer.make_token(TokenTypes::ImplStart)
            }
        } else if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
            parse_ident(tokenizer, TokenTypes::FieldName, &[b':', b'='])
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::FieldName => if tokenizer.matches(":") {
            tokenizer.make_token(TokenTypes::FieldSeparator)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::FieldSeparator =>
            parse_ident(tokenizer, TokenTypes::FieldType, &[b'=', b';']),
        TokenTypes::FieldType => if tokenizer.matches("=") {
            if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
                tokenizer.state = TokenizerState::CODE_TO_STRUCT_TOP;
            } else {
                tokenizer.state = TokenizerState::CODE;
            }
            tokenizer.make_token(TokenTypes::FieldValue)
        } else if tokenizer.matches(";") {
            tokenizer.make_token(TokenTypes::FieldEnd)
        } else {
            tokenizer.handle_invalid()
        },
        TokenTypes::Identifier => if tokenizer.matches(";") {
            tokenizer.make_token(TokenTypes::ImportEnd)
        } else {
            tokenizer.handle_invalid()
        }
        _ => {
            if tokenizer.matches("import") {
                tokenizer.make_token(TokenTypes::ImportStart)
            } else if tokenizer.matches("}") && tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
                tokenizer.state = TokenizerState::TOP_ELEMENT;
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
        TokenTypes::Identifier => if tokenizer.matches("<") {
            if tokenizer.state == TokenizerState::FUNCTION {
                tokenizer.state = TokenizerState::GENERIC_TO_FUNC;
            } else {
                tokenizer.state = TokenizerState::GENERIC_TO_FUNC_TOP;
            }
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.matches("(") {
            tokenizer.make_token(TokenTypes::ArgumentsStart)
        } else {
            tokenizer.state = TokenizerState::TOP_ELEMENT;
            tokenizer.handle_invalid()
        },
        TokenTypes::GenericEnd => {
            if !tokenizer.matches("(") {
                tokenizer.handle_invalid()
            } else {
                tokenizer.make_token(TokenTypes::ArgumentsStart)
            }
        }
        TokenTypes::ArgumentsStart | TokenTypes::ArgumentEnd => if tokenizer.matches(")") {
            tokenizer.make_token(TokenTypes::ArgumentsEnd)
        } else {
            parse_ident(tokenizer, TokenTypes::ArgumentName, &[b':', b','])
        },
        TokenTypes::ArgumentName => if tokenizer.matches(":") {
            tokenizer.make_token(TokenTypes::ArgumentTypeSeparator)
        } else {
            //Skip the comma if there is one
            if tokenizer.matches(",") {
                tokenizer.make_token(TokenTypes::ArgumentSeparator)
            } else {
                tokenizer.make_token(TokenTypes::ArgumentEnd)
            }
        },
        TokenTypes::ArgumentTypeSeparator =>
            parse_ident(tokenizer, TokenTypes::ArgumentType, &[b',', b')']),
        TokenTypes::ArgumentType => if tokenizer.matches(",") {
            tokenizer.make_token(TokenTypes::ArgumentSeparator)
        } else {
            tokenizer.make_token(TokenTypes::ArgumentEnd)
        },
        TokenTypes::ArgumentSeparator => tokenizer.make_token(TokenTypes::ArgumentEnd),
        TokenTypes::ReturnTypeArrow => {
            parse_ident(tokenizer, TokenTypes::ReturnType, &[b';', b'{'])
        }
        TokenTypes::ArgumentsEnd | TokenTypes::ReturnType =>
            if tokenizer.last.token_type == TokenTypes::ArgumentsEnd && tokenizer.matches("->") {
                tokenizer.make_token(TokenTypes::ReturnTypeArrow)
            } else if tokenizer.matches("{") {
                tokenizer.state = TokenizerState::CODE;
                tokenizer.make_token(TokenTypes::CodeStart)
            } else if tokenizer.matches(";") {
                if tokenizer.state == TokenizerState::FUNCTION {
                    tokenizer.state = TokenizerState::TOP_ELEMENT;
                } else if tokenizer.state == TokenizerState::FUNCTION_TO_STRUCT_TOP {
                    tokenizer.state = TokenizerState::TOP_ELEMENT_TO_STRUCT;
                }
                tokenizer.make_token(TokenTypes::CodeEnd)
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
        TokenTypes::StructStart | TokenTypes::TraitStart => parse_ident(tokenizer, TokenTypes::Identifier, &[b'{', b'<']),
        TokenTypes::Identifier | TokenTypes::GenericEnd => if tokenizer.matches("<") {
            tokenizer.state = TokenizerState::GENERIC_TO_STRUCT;
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.matches("{") {
            tokenizer.state = TokenizerState::TOP_ELEMENT_TO_STRUCT;
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
            tokenizer.state = TokenizerState::GENERIC_TO_IMPL;
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else {
            tokenizer.parse_to_first(TokenTypes::Identifier, b'<', b' ')
        },
        TokenTypes::GenericEnd => if tokenizer.matches("for") {
            tokenizer.state = TokenizerState::STRUCTURE;
            tokenizer.make_token(TokenTypes::TraitStart)
        } else {
            tokenizer.next_included()?;
            tokenizer.parse_to_first(TokenTypes::Identifier, b'<', b' ')
        }
        TokenTypes::Identifier => if tokenizer.matches("<") {
            tokenizer.state = TokenizerState::GENERIC_TO_IMPL;
            tokenizer.make_token(TokenTypes::GenericsStart)
        } else if tokenizer.matches("for") {
            tokenizer.state = TokenizerState::STRUCTURE;
            tokenizer.make_token(TokenTypes::TraitStart)
        } else {
            tokenizer.state = TokenizerState::TOP_ELEMENT;
            tokenizer.handle_invalid()
        },
        token => panic!("How'd you get here? {:?}", token)
    }
}

#[cfg(test)]
mod tests {
    use crate::tokens::util::check_types;
    use super::*;

    #[test]
    fn test_eof() {
        let mut types = Vec::new();
        add_header(0, &mut types);
        types.push(TokenTypes::EOF);
        let testing = "";
        check_types(&types, testing, TokenizerState::TOP_ELEMENT);
    }

    #[test]
    fn test_func() {
        let mut types = Vec::new();
        add_header(2, &mut types);
        types.push(TokenTypes::FunctionStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, true, &mut types);
        add_arguments(2, true, &mut types);
        types.push(TokenTypes::ReturnType);
        types.push(TokenTypes::CodeStart);
        let testing = "pub internal fn testing<T: Bound>(self, arg2: TypeAgain) -> ReturnType {}";
        check_types(&types, testing, TokenizerState::TOP_ELEMENT);
    }

    #[test]
    fn test_struct() {
        let mut types = Vec::new();
        //Testing
        add_header(1, &mut types);
        types.push(TokenTypes::TraitStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, true, &mut types);
        //trait_func
        types.push(TokenTypes::StructTopElement);
        add_header(1, &mut types);
        types.push(TokenTypes::FunctionStart);
        types.push(TokenTypes::Identifier);
        add_arguments(0, false, &mut types);
        types.push(TokenTypes::StructEnd);
        //TestStruct
        add_header(1, &mut types);
        types.push(TokenTypes::StructStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, true, &mut types);
        types.push(TokenTypes::StructTopElement);
        //field
        add_header(1, &mut types);
        types.push(TokenTypes::FieldName);
        types.push(TokenTypes::FieldType);
        types.push(TokenTypes::FieldEnd);
        types.push(TokenTypes::StructEnd);
        //impl
        add_header(0, &mut types);
        types.push(TokenTypes::ImplStart);
        add_generics(2, true, &mut types);
        types.push(TokenTypes::Identifier);
        add_generics(1, false, &mut types);
        types.push(TokenTypes::TraitStart);
        types.push(TokenTypes::Identifier);
        add_generics(1, false, &mut types);
        types.push(TokenTypes::StructTopElement);
        //test_func
        add_header(1, &mut types);
        types.push(TokenTypes::FunctionStart);
        types.push(TokenTypes::Identifier);
        add_arguments(0, false, &mut types);
        types.push(TokenTypes::CodeStart);

        let testing = "pub trait Testing<T: Bound> {\
            pub fn trait_func();\
        }\
        pub struct TestStruct<T: OtherBound> {\
            pub field: MyField;\
        }\
        impl<T: Bound, E: OtherBound> Test<T> for TestStruct<E> {\
            pub fn trait_func() {}\
        }";
        check_types(&types, testing, TokenizerState::TOP_ELEMENT);
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