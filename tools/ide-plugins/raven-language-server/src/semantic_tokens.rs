use crossbeam_channel::Sender;
use lsp_server::{Message, RequestId, Response};
use lsp_types::{SemanticToken, SemanticTokens, SemanticTokensResult};

use parser::tokens::tokenizer::Tokenizer;
use parser::tokens::tokens::{Token, TokenTypes};

/// Parses the semantic tokens from the file and sends it to the IDE
pub async fn parse_semantic_tokens(id: RequestId, file: String, sender: Sender<Message>) {
    // Gets the tokens
    let mut tokenizer = Tokenizer::new(file.as_bytes());
    let mut tokens = Vec::new();
    loop {
        tokens.push(tokenizer.next());
        if tokens.last().unwrap().token_type == TokenTypes::EOF {
            break;
        }
    }

    // Converts the tokens into semantic tokens
    let mut last: Option<Token> = None;
    let data = tokens
        .into_iter()
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
        TokenTypes::Identifier | TokenTypes::ReturnType => SemanticTokenTypes::Type,
        TokenTypes::Variable => SemanticTokenTypes::Property,
        TokenTypes::Modifier => SemanticTokenTypes::Keyword,
        TokenTypes::Comment => SemanticTokenTypes::Comment,
        TokenTypes::ImportStart | TokenTypes::Return | TokenTypes::New | TokenTypes::FunctionStart => {
            SemanticTokenTypes::Keyword
        }
        TokenTypes::StringStart | TokenTypes::StringEnd | TokenTypes::StringEscape => SemanticTokenTypes::String,
        _ => SemanticTokenTypes::None,
    } as u32;
    return temp;
}

/// The integer ids of the semantic tokens along with the VSCode color
#[allow(dead_code)]
pub enum SemanticTokenTypes {
    Namespace = 0,     // Same as type
    Type = 1,          // Blue-green color
    Class = 2,         // Same as type
    Enum = 3,          // Same as type
    Interface = 4,     // Same as type
    Struct = 5,        // Same as type
    TypeParameter = 6, // Same color as Type
    Parameter = 7,     // Same color as Property
    Variable = 8,      // Same color as Property
    Property = 9,      // Light blue
    EnumMember = 10,   // Blue
    Event = 11,        // Same color as Property
    Function = 12,     // Yellow
    Method = 13,       // Same as type
    Macro = 14,        // Same as Property
    Keyword = 15,      // Purple
    Modifier = 16,     // White
    Comment = 17,      // Green
    String = 18,       // Orange
    Number = 19,       // Green-Yellow
    Regexp = 20,       // Dark blue color
    Operator = 21,     // White
    Decorator = 22,    // Same color as Function
    None = 100,
}
