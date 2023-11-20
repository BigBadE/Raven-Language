use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::util::{parse_attribute_val, parse_modifier, parse_to_character};

/// Handles when the tokenizer isn't in any other state.
pub fn next_top_token(tokenizer: &mut Tokenizer) -> Token {
    if tokenizer.index == tokenizer.len {
        return tokenizer.make_token(TokenTypes::EOF);
    }

    return match tokenizer.last.token_type {
        TokenTypes::ImportStart => parse_to_character(tokenizer, TokenTypes::Identifier, &[b';']),
        // Each attribute is in the format #[name(value)] or #[name], this confirms the ] at the end.
        TokenTypes::Attribute => {
            if tokenizer.matches("]") {
                tokenizer.make_token(TokenTypes::AttributeEnd)
            } else {
                tokenizer.handle_invalid()
            }
        }
        TokenTypes::AttributesStart | TokenTypes::AttributeEnd => {
            if tokenizer.matches("#[") {
                tokenizer.make_token(TokenTypes::AttributeStart)
            } else {
                // If there aren't attributes, move on to modifiers
                tokenizer.make_token(TokenTypes::ModifiersStart)
            }
        }
        TokenTypes::AttributeStart => parse_attribute_val(tokenizer, TokenTypes::Attribute),
        // Check for chained modifiers
        TokenTypes::ModifiersStart | TokenTypes::Modifier => get_top_element(tokenizer),
        TokenTypes::FieldName => {
            if tokenizer.matches(":") {
                tokenizer.make_token(TokenTypes::FieldSeparator)
            } else {
                tokenizer.handle_invalid()
            }
        }
        TokenTypes::FieldSeparator => {
            parse_to_character(tokenizer, TokenTypes::FieldType, &[b'=', b';'])
        }
        TokenTypes::FieldType => {
            if tokenizer.matches("=") {
                // Handles the code for the field's value
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
            }
        }
        TokenTypes::Identifier => {
            if tokenizer.matches(";") {
                tokenizer.make_token(TokenTypes::ImportEnd)
            } else {
                tokenizer.handle_invalid()
            }
        }
        _ => {
            if tokenizer.matches("import") {
                tokenizer.make_token(TokenTypes::ImportStart)
            } else if tokenizer.matches("}")
                && tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT
            {
                // Handles the end of the struct
                tokenizer.state = TokenizerState::TOP_ELEMENT;
                tokenizer.make_token(TokenTypes::StructEnd)
            } else {
                tokenizer.make_token(TokenTypes::AttributesStart)
            }
        }
    };
}

fn get_top_element(tokenizer: &mut Tokenizer) -> Token {
    return if let Some(modifier) = parse_modifier(tokenizer) {
        modifier
    } else if tokenizer.matches("fn") {
        // Find the correct function state
        if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
            tokenizer.state = TokenizerState::FUNCTION_TO_STRUCT_TOP;
        } else {
            tokenizer.state = TokenizerState::FUNCTION;
        }
        tokenizer.make_token(TokenTypes::FunctionStart)
    } else if tokenizer.matches("struct") {
        // Structs can't be inside structures
        if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
            tokenizer.handle_invalid()
        } else {
            tokenizer.state = TokenizerState::STRUCTURE;
            tokenizer.make_token(TokenTypes::StructStart)
        }
    } else if tokenizer.matches("trait") {
        // Traits can't be inside structures
        if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
            tokenizer.handle_invalid()
        } else {
            tokenizer.state = TokenizerState::STRUCTURE;
            tokenizer.make_token(TokenTypes::TraitStart)
        }
    } else if tokenizer.matches("impl") {
        // What is being implemented is next, so whitespace is skipped.
        tokenizer.next_included().unwrap_or(0);
        tokenizer.index -= 1;

        if tokenizer.buffer[tokenizer.index] == b' ' {
            tokenizer.index += 1;
        }

        // Impls can't be inside structures
        if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
            tokenizer.handle_invalid()
        } else {
            tokenizer.state = TokenizerState::IMPLEMENTATION;
            tokenizer.make_token(TokenTypes::ImplStart)
        }
    } else if tokenizer.state == TokenizerState::TOP_ELEMENT_TO_STRUCT {
        // Looking for a field name inside a struct
        parse_to_character(tokenizer, TokenTypes::FieldName, &[b':', b'='])
    } else if tokenizer.state == TokenizerState::TOP_ELEMENT && tokenizer.matches("") {
        // If there are blank lines after all of the modifiers at the EOF, ignore them
        tokenizer.index += 1;
        tokenizer.make_token(TokenTypes::BlankLine)
    } else {
        tokenizer.handle_invalid()
    };
}

/// Handles when the tokenizer is parsing the header of a struct
pub fn next_func_token(tokenizer: &mut Tokenizer) -> Token {
    return match &tokenizer.last.token_type {
        TokenTypes::FunctionStart => {
            parse_to_character(tokenizer, TokenTypes::Identifier, &[b'<', b'('])
        }
        TokenTypes::Identifier => {
            if tokenizer.matches("<") {
                // Handles the generics after the function's name, if it exists
                if tokenizer.state == TokenizerState::FUNCTION {
                    tokenizer.state = TokenizerState::GENERIC_TO_FUNC;
                } else {
                    tokenizer.state = TokenizerState::GENERIC_TO_FUNC_TO_STRUCT_TOP;
                }
                tokenizer.make_token(TokenTypes::GenericsStart)
            } else if tokenizer.matches("(") {
                // If no generics, it must be arguments
                tokenizer.make_token(TokenTypes::ArgumentsStart)
            } else {
                tokenizer.state = TokenizerState::TOP_ELEMENT;
                tokenizer.handle_invalid()
            }
        }
        TokenTypes::GenericsEnd => {
            // After generics, it's arguments
            if tokenizer.matches("(") {
                tokenizer.make_token(TokenTypes::ArgumentsStart)
            } else {
                tokenizer.handle_invalid()
            }
        }
        // Check if arguments are done
        TokenTypes::ArgumentsStart | TokenTypes::ArgumentEnd => {
            if tokenizer.matches(")") {
                tokenizer.make_token(TokenTypes::ArgumentsEnd)
            } else {
                parse_to_character(tokenizer, TokenTypes::ArgumentName, &[b':', b',', b')'])
            }
        }
        TokenTypes::ArgumentName => {
            if tokenizer.matches(":") {
                tokenizer.make_token(TokenTypes::ArgumentTypeSeparator)
            } else {
                //Skip the comma if there is one
                if tokenizer.matches(",") {
                    tokenizer.make_token(TokenTypes::ArgumentSeparator)
                } else {
                    tokenizer.make_token(TokenTypes::ArgumentEnd)
                }
            }
        }
        TokenTypes::ArgumentTypeSeparator => {
            parse_to_character(tokenizer, TokenTypes::ArgumentType, &[b',', b')'])
        }
        TokenTypes::ArgumentType => {
            if tokenizer.matches(",") {
                tokenizer.make_token(TokenTypes::ArgumentSeparator)
            } else {
                tokenizer.make_token(TokenTypes::ArgumentEnd)
            }
        }
        TokenTypes::ArgumentSeparator => tokenizer.make_token(TokenTypes::ArgumentEnd),
        // Parse the return type
        TokenTypes::ReturnTypeArrow => {
            parse_to_character(tokenizer, TokenTypes::ReturnType, &[b';', b'{'])
        }
        TokenTypes::ArgumentsEnd | TokenTypes::ReturnType => get_return_token(tokenizer),
        token => {
            panic!("How'd you get here? {:?}", token);
        }
    };
}

pub fn get_return_token(tokenizer: &mut Tokenizer) -> Token {
    if tokenizer.last.token_type == TokenTypes::ArgumentsEnd && tokenizer.matches("->") {
        tokenizer.make_token(TokenTypes::ReturnTypeArrow)
    } else if tokenizer.matches("{") {
        if tokenizer.state == TokenizerState::FUNCTION_TO_STRUCT_TOP {
            tokenizer.state = TokenizerState::CODE_TO_STRUCT_TOP;
        } else {
            tokenizer.state = TokenizerState::CODE;
        }
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
}

/// Finds the next token in the struct header. This only handles generics, implemented types,
/// structure name, and the start of the code.
pub fn next_struct_token(tokenizer: &mut Tokenizer) -> Token {
    match tokenizer.last.token_type {
        TokenTypes::StructStart | TokenTypes::TraitStart | TokenTypes::For => {
            parse_to_character(tokenizer, TokenTypes::Identifier, &[b'{', b'<'])
        }
        TokenTypes::Identifier | TokenTypes::GenericsEnd => {
            if tokenizer.matches("<") {
                tokenizer.state = TokenizerState::GENERIC_TO_STRUCT;
                tokenizer.make_token(TokenTypes::GenericsStart)
            } else if tokenizer.matches("{") {
                tokenizer.state = TokenizerState::TOP_ELEMENT_TO_STRUCT;
                tokenizer.make_token(TokenTypes::StructTopElement)
            } else {
                tokenizer.handle_invalid()
            }
        }
        _ => panic!("How'd you get here? {:?}", tokenizer.last.token_type),
    }
}

/// Gets the next token of the implementation.
/// This ends at the "for" keyword.
pub fn next_implementation_token(tokenizer: &mut Tokenizer) -> Token {
    match &tokenizer.last.token_type {
        TokenTypes::ImplStart => {
            if tokenizer.matches("<") {
                tokenizer.state = TokenizerState::GENERIC_TO_IMPL;
                tokenizer.make_token(TokenTypes::GenericsStart)
            } else {
                tokenizer.parse_to_first(TokenTypes::Identifier, b'<', b' ')
            }
        }
        TokenTypes::GenericsEnd => {
            if tokenizer.matches("for") {
                tokenizer.state = TokenizerState::STRUCTURE;
                tokenizer.make_token(TokenTypes::For)
            } else {
                tokenizer.next_included()?;
                tokenizer.parse_to_first(TokenTypes::Identifier, b'<', b' ')
            }
        }
        TokenTypes::Identifier => {
            if tokenizer.matches("<") {
                tokenizer.state = TokenizerState::GENERIC_TO_IMPL;
                tokenizer.make_token(TokenTypes::GenericsStart)
            } else if tokenizer.matches("for") {
                tokenizer.state = TokenizerState::STRUCTURE;
                tokenizer.make_token(TokenTypes::For)
            } else {
                tokenizer.state = TokenizerState::TOP_ELEMENT;
                tokenizer.last.to_string(tokenizer.buffer);
                tokenizer.handle_invalid()
            }
        }
        token => panic!("How'd you get here? {:?}", token),
    }
}
