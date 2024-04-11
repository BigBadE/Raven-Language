use crossbeam_channel::Sender;
use lsp_server::{Message, RequestId, Response};
use lsp_types::{SemanticToken, SemanticTokens, SemanticTokensResult};

use data::tokens::{Token, TokenTypes};
use parser::tokens::tokenizer::Tokenizer;

/// Parses the semantic tokens from the file and sends it to the IDE
pub async fn parse_semantic_tokens(id: RequestId, file: String, sender: Sender<Message>) {
    // Converts the tokens into semantic tokens
    let mut last: Option<Token> = None;
    let data = TokenIterator::new(file.as_bytes())
        .map(|mut token| {
            // Multi-line tokens aren't supported, set the end to the start
            if token.start.0 != token.end.0 {
                token.start_offset = token.end_offset - token.end.1 as usize;
                token.start = (token.end.0, 0);
            }

            // Calculate the semantic token data as the LSP protocol requires
            let delta_line = (token.start.0 - 1) - last.clone().map_or(0, |inner| inner.start.0 - 1);
            let temp = SemanticToken {
                delta_line,
                delta_start: if delta_line == 0 {
                    token.start_offset - last.clone().map_or(0, |inner| inner.start_offset)
                } else {
                    0
                } as u32,
                length: (token.end_offset - token.start_offset) as u32,
                token_type: get_token(last.as_ref().map_or(&TokenTypes::EOF, |inner| &inner.token_type), &token.token_type),
                token_modifiers_bitset: 0,
            };

            last = Some(token);
            temp
        })
        .collect::<Vec<_>>();

    // Send the response back to the IDE
    let result = Some(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data }));
    let result = serde_json::to_value(&result).unwrap();
    let resp = Response { id, result: Some(result), error: None };
    sender.send(Message::Response(resp)).unwrap();
}

pub struct TokenIterator<'a> {
    tokenizer: Tokenizer<'a>,
    ended: bool,
}

impl<'a> TokenIterator<'a> {
    pub fn new(file: &'a [u8]) -> Self {
        return TokenIterator { tokenizer: Tokenizer::new(file), ended: false };
    }
}

impl Iterator for TokenIterator<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let token = self.tokenizer.next();
        if token.token_type == TokenTypes::EOF {
            self.ended = true;
        }
        return Some(token);
    }
}

/// Gets the token's type from the last token and the current token
fn get_token(last: &TokenTypes, token_type: &TokenTypes) -> u32 {
    match *last {
        TokenTypes::FunctionStart => {
            if *token_type == TokenTypes::Identifier {
                return SemanticTokenTypes::Function as u32;
            }
        }
        TokenTypes::ImportStart => return SemanticTokenTypes::Property as u32,
        _ => {}
    }
    let temp = match token_type {
        TokenTypes::Identifier
        | TokenTypes::ReturnType
        | TokenTypes::ArgumentName
        | TokenTypes::ReturnTypeArrow
        | TokenTypes::Generic => SemanticTokenTypes::Type,
        TokenTypes::Variable | TokenTypes::ImplStart | TokenTypes::TraitStart => SemanticTokenTypes::Property,
        TokenTypes::Modifier
        | TokenTypes::GenericsStart
        | TokenTypes::GenericsEnd
        | TokenTypes::GenericBound
        | TokenTypes::GenericBoundEnd
        | TokenTypes::For
        | TokenTypes::If
        | TokenTypes::True
        | TokenTypes::False
        | TokenTypes::Else
        | TokenTypes::Do
        | TokenTypes::While
        | TokenTypes::In => SemanticTokenTypes::Keyword,
        TokenTypes::Comment => SemanticTokenTypes::Comment,
        TokenTypes::ImportStart | TokenTypes::Return | TokenTypes::New | TokenTypes::FunctionStart | TokenTypes::Let => {
            SemanticTokenTypes::Keyword
        }
        TokenTypes::Integer | TokenTypes::Float => SemanticTokenTypes::Number,
        TokenTypes::CallingType | TokenTypes::Attribute => SemanticTokenTypes::Function,
        TokenTypes::StringStart | TokenTypes::StringEnd | TokenTypes::StringEscape => SemanticTokenTypes::String,
        _ => SemanticTokenTypes::None,
    } as u32;
    return temp;
}

/// The integer ids of the semantic tokens along with the VSCode color
#[allow(dead_code)]
pub enum SemanticTokenTypes {
    /// Same as Type
    Namespace = 0,
    /// Blue-green color
    Type = 1,
    /// Same as Type
    Class = 2,
    /// Same as Type
    Enum = 3,
    /// Same as Type
    Interface = 4,
    /// Same as Type
    Struct = 5,
    /// Same as Type
    TypeParameter = 6,
    /// Same as Property
    Parameter = 7,
    /// Same as Property
    Variable = 8,
    /// Light blue
    Property = 9,
    /// Blue
    EnumMember = 10,
    /// Same as Property
    Event = 11,
    /// Yellow
    Function = 12,
    /// Same as Type
    Method = 13,
    /// Same as Property
    Macro = 14,
    /// Purple
    Keyword = 15,
    /// White
    Modifier = 16,
    /// Green
    Comment = 17,
    /// Orange
    String = 18,
    /// Green-Yellow
    Number = 19,
    /// Dark blue color
    Regexp = 20,
    /// White
    Operator = 21,
    /// Same color as Function
    Decorator = 22,
    /// No color
    None = 100,
}
