use std::fs;
use crossbeam_channel::Sender;
use lsp_server::{Message, RequestId, Response};
use lsp_types::{SemanticToken, SemanticTokens, SemanticTokensResult, Url};
use parser::tokens::tokenizer::Tokenizer;
use parser::tokens::tokens::{Token, TokenTypes};
use urlencoding::decode;

pub async fn parse_semantic_tokens(id: RequestId, file: Url, sender: Sender<Message>) {
    let mut url = decode(file.path()).unwrap().to_string();
    url.remove(0);

    let buffer = fs::read(url).unwrap();
    let mut tokenizer = Tokenizer::new(buffer.as_slice());
    let mut tokens = Vec::new();
    loop {
        tokens.push(tokenizer.next());
        if tokens.last().unwrap().token_type == TokenTypes::EOF {
            break
        }
    }

    let mut last = None;
    let data = tokens.iter().map(|token| {
        eprintln!("Line ({}, {}) to ({}, {}) for {:?}", token.start.0, token.start.1, token.end.0, token.end.1, token.token_type);
        let delta_line = (token.start.0 - 1) - last.map(|inner: &Token| inner.start.0 - 1).unwrap_or(0);
        let temp = SemanticToken {
            delta_line,
            delta_start: if delta_line == 0 {
                token.start_offset - last.map(|inner| inner.start_offset).unwrap_or(0)
            } else {
                0
            } as u32 * 2,
            length: (token.end_offset - token.start_offset) as u32,
            token_type: get_token(&token.token_type),
            token_modifiers_bitset: 0,
        };
        last = Some(token);
        temp
    }).collect::<Vec<_>>();
    let result = Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }));
    let result = serde_json::to_value(&result).unwrap();
    let resp = Response { id, result: Some(result), error: None };
    sender.send(Message::Response(resp)).unwrap();
}

fn get_token(token_type: &TokenTypes) -> u32 {
    return match token_type {
        TokenTypes::Comment => 1,
        _ => 0
    };
}