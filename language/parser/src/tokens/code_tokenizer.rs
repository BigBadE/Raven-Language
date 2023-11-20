use crate::tokens::tokenizer::{Tokenizer, TokenizerState};
use crate::tokens::tokens::{Token, TokenTypes};
use crate::tokens::util::{parse_acceptable, parse_numbers};

/// Gets the next token in a block of code.
pub fn next_code_token(tokenizer: &mut Tokenizer) -> Token {
    return if let Some(found) = check_keywords(tokenizer) {
        found
    } else if TokenTypes::Period == tokenizer.last.token_type
        && tokenizer.buffer[tokenizer.index].is_ascii_alphabetic()
    {
        parse_acceptable(tokenizer, TokenTypes::CallingType)
    } else if tokenizer.matches("{") {
        tokenizer.bracket_depth += 1;
        tokenizer.make_token(TokenTypes::BlockStart)
    } else if tokenizer.matches("}") {
        if tokenizer.bracket_depth == 0 {
            // If it's the last matching bracket, then end the code block.
            if tokenizer.state == TokenizerState::CODE_TO_STRUCT_TOP {
                tokenizer.state = TokenizerState::TOP_ELEMENT_TO_STRUCT;
            } else {
                tokenizer.state = TokenizerState::TOP_ELEMENT;
            }
            tokenizer.make_token(TokenTypes::CodeEnd)
        } else {
            // There's another bracket, so this is just the end of the block.
            tokenizer.bracket_depth -= 1;
            tokenizer.make_token(TokenTypes::BlockEnd)
        }
    } else if tokenizer.matches(".") {
        // This is only a number if the thing before and after is a digit. "1." and ".1" aren't numbers.
        if tokenizer.buffer[tokenizer.index].is_ascii_digit()
            && tokenizer.buffer[tokenizer.index - 2].is_ascii_digit()
        {
            tokenizer.index -= 1;
            parse_numbers(tokenizer)
        } else {
            tokenizer.make_token(TokenTypes::Period)
        }
    } else if tokenizer.matches("\"") {
        // Changes the state type based on what the current state already is.
        tokenizer.state = if tokenizer.state == TokenizerState::CODE {
            TokenizerState::STRING
        } else {
            TokenizerState::STRING_TO_CODE_STRUCT_TOP
        };
        tokenizer.make_token(TokenTypes::StringStart)
    } else if tokenizer.matches("'") {
        tokenizer.index += 1;
        if tokenizer.matches("'") {
            tokenizer.make_token(TokenTypes::Char)
        } else {
            tokenizer.handle_invalid()
        }
    } else {
        let found = tokenizer.next_included()?;
        if tokenizer.matches("//") {
            tokenizer.parse_to_line_end(TokenTypes::Comment)
        } else if (found as char).is_alphabetic() || found == b'_' {
            // A character or an underscore is a variable.
            let temp = parse_acceptable(tokenizer, TokenTypes::Variable);
            temp
        } else if found >= b'0' && found <= b'9' {
            // A number is a number.
            parse_numbers(tokenizer)
        } else {
            // Everything else is an operator.
            tokenizer.make_token(TokenTypes::Operator)
        }
    };
}

pub fn check_keywords(tokenizer: &mut Tokenizer) -> Option<Token> {
    return Some(if tokenizer.matches(";") {
        tokenizer.make_token(TokenTypes::LineEnd)
    } else if tokenizer.matches(",") {
        tokenizer.make_token(TokenTypes::ArgumentEnd)
    } else if tokenizer.matches("(") {
        tokenizer.make_token(TokenTypes::ParenOpen)
    } else if tokenizer.matches(")") {
        tokenizer.make_token(TokenTypes::ParenClose)
    } else if tokenizer.matches_word("return") {
        tokenizer.make_token(TokenTypes::Return)
    } else if tokenizer.matches_word("break") {
        tokenizer.make_token(TokenTypes::Break)
    } else if tokenizer.matches_word("switch") {
        tokenizer.make_token(TokenTypes::Switch)
    } else if tokenizer.matches_word("true") {
        tokenizer.make_token(TokenTypes::True)
    } else if tokenizer.matches_word("false") {
        tokenizer.make_token(TokenTypes::False)
    } else if tokenizer.matches_word("for") {
        tokenizer.make_token(TokenTypes::For)
    } else if tokenizer.matches_word("while") {
        tokenizer.make_token(TokenTypes::While)
    } else if tokenizer.matches_word("new") {
        tokenizer.make_token(TokenTypes::New)
    } else if tokenizer.matches_word("if") {
        tokenizer.make_token(TokenTypes::If)
    } else if tokenizer.matches_word("do") {
        tokenizer.make_token(TokenTypes::Do)
    } else if tokenizer.matches_word("else") {
        tokenizer.make_token(TokenTypes::Else)
    } else if tokenizer.matches_word("in") {
        tokenizer.make_token(TokenTypes::In)
    } else if tokenizer.matches(":") {
        tokenizer.make_token(TokenTypes::Colon)
    } else if tokenizer.matches_word("let") {
        tokenizer.make_token(TokenTypes::Let)
    } else if tokenizer.matches("=") {
        tokenizer.make_token(TokenTypes::Equals)
    } else {
        return None;
    });
}
