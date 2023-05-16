use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::util::{parse_acceptable, parse_numbers};

pub fn next_code_token(tokenizer: &mut Tokenizer, bracket_depth: u64) -> Token {
    if tokenizer.matches(";") {
        tokenizer.for_loop = false;
        if (tokenizer.state.clone() as u64 & TokenizerState::CODE_TO_STRUCT_TOP as u64) != 12 {
            tokenizer.state = TokenizerState::TOP_ELEMENT_TO_STRUCT;
            tokenizer.make_token(TokenTypes::CodeEnd)
        } else {
            tokenizer.make_token(TokenTypes::LineEnd)
        }
    } else if tokenizer.matches("{") {
        tokenizer.state += 0x100;
        tokenizer.make_token(TokenTypes::BlockStart)
    } else if tokenizer.matches("}") {
        tokenizer.for_loop = false;
        if bracket_depth == 0 {
            if (tokenizer.state.clone() as u64 & TokenizerState::CODE_TO_STRUCT_TOP as u64) != 0 {
                tokenizer.state = TokenizerState::TOP_ELEMENT_TO_STRUCT;
            } else {
                tokenizer.state = TokenizerState::TOP_ELEMENT;
            }
            tokenizer.make_token(TokenTypes::CodeEnd)
        } else {
            tokenizer.state -= 0x100;
            tokenizer.make_token(TokenTypes::BlockEnd)
        }
    } else if tokenizer.matches(",") {
        tokenizer.make_token(TokenTypes::ArgumentEnd)
    } else if tokenizer.matches("(") {
        tokenizer.make_token(TokenTypes::ParenOpen)
    } else if tokenizer.matches(")") {
        tokenizer.make_token(TokenTypes::ParenClose)
    } else if tokenizer.matches(".") {
        if tokenizer.len == tokenizer.index {
            return tokenizer.make_token(TokenTypes::EOF);
        }
        if (tokenizer.buffer[tokenizer.index] as char).is_numeric() {
            tokenizer.index -= 1;
            parse_numbers(tokenizer)
        } else {
            parse_acceptable(tokenizer, TokenTypes::CallingType)
        }
    } else if tokenizer.matches("return") {
        tokenizer.make_token(TokenTypes::Return)
    } else if tokenizer.matches("break") {
        tokenizer.make_token(TokenTypes::Break)
    } else if tokenizer.matches("switch") {
        tokenizer.make_token(TokenTypes::Switch)
    } else if tokenizer.matches("for") &&
        (tokenizer.last.token_type == TokenTypes::LineEnd || tokenizer.last.token_type == TokenTypes::CodeEnd) {
        tokenizer.for_loop = true;
        tokenizer.make_token(TokenTypes::For)
    } else if tokenizer.matches("new") {
        tokenizer.make_token(TokenTypes::New)
    } else if tokenizer.matches("while") &&
        (tokenizer.last.token_type == TokenTypes::LineEnd || tokenizer.last.token_type == TokenTypes::CodeEnd){
        tokenizer.make_token(TokenTypes::While)
    } else if tokenizer.matches("if") {
        tokenizer.make_token(TokenTypes::If)
    } else if tokenizer.matches("else") {
        tokenizer.make_token(TokenTypes::Else)
    } else if tokenizer.matches("in") && tokenizer.for_loop {
        tokenizer.for_loop = false;
        tokenizer.make_token(TokenTypes::In)
    } else if tokenizer.matches(":") {
        tokenizer.make_token(TokenTypes::Colon)
    } else if tokenizer.matches("let") {
        if tokenizer.buffer[tokenizer.index].is_ascii_alphanumeric() {
            tokenizer.index -= 3;
            parse_acceptable(tokenizer, TokenTypes::Variable)
        } else {
            tokenizer.make_token(TokenTypes::Let)
        }
    } else if tokenizer.matches("=") {
        tokenizer.make_token(TokenTypes::Equals)
    } else if tokenizer.matches("\"") {
        tokenizer.state = tokenizer.state & 0xFF;
        tokenizer.make_token(TokenTypes::StringStart)
    } else {
        let found = tokenizer.next_included()?;
        if (found as char).is_alphabetic() || found == b'_' {
            parse_acceptable(tokenizer, TokenTypes::Variable)
        } else if found >= b'0' && found <= b'9' {
            parse_numbers(tokenizer)
        } else {
            tokenizer.make_token(TokenTypes::Operator)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::tokens::util::check_types;

    use super::*;

    #[test]
    fn test_code() {
        let types = [TokenTypes::If, TokenTypes::ParenOpen, TokenTypes::Integer,
            TokenTypes::Operator, TokenTypes::Float, TokenTypes::ParenClose, TokenTypes::CallingType, TokenTypes::ParenOpen,
            TokenTypes::Variable, TokenTypes::ArgumentEnd, TokenTypes::Variable, TokenTypes::ParenClose, TokenTypes::CodeStart];
        let code = "if (1 + 2.2).function(arg, args) {\
        for testing in test {\
        while \"my_str\\\"continues!\"{\
        return something;\
        }\
        }\
        }";
        check_types(&types, code, TokenizerState::CODE);
    }
}