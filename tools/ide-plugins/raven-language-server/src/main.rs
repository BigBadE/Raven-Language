use std::collections::HashMap;
use std::error::Error;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId};
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument};
use lsp_types::request::SemanticTokensFullRequest;
use lsp_types::{
    InitializeParams, SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    WorkDoneProgressOptions,
};
use tokio::runtime::Builder;

use crate::semantic_tokens::parse_semantic_tokens;

mod semantic_tokens;

pub fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                work_done_progress_options: WorkDoneProgressOptions::default(),
                legend: SemanticTokensLegend {
                    token_types: vec![
                        SemanticTokenType::NAMESPACE,
                        SemanticTokenType::TYPE,
                        SemanticTokenType::CLASS,
                        SemanticTokenType::ENUM,
                        SemanticTokenType::INTERFACE,
                        SemanticTokenType::STRUCT,
                        SemanticTokenType::TYPE_PARAMETER,
                        SemanticTokenType::PARAMETER,
                        SemanticTokenType::VARIABLE,
                        SemanticTokenType::PROPERTY,
                        SemanticTokenType::ENUM_MEMBER,
                        SemanticTokenType::EVENT,
                        SemanticTokenType::FUNCTION,
                        SemanticTokenType::METHOD,
                        SemanticTokenType::MACRO,
                        SemanticTokenType::KEYWORD,
                        SemanticTokenType::MODIFIER,
                        SemanticTokenType::COMMENT,
                        SemanticTokenType::STRING,
                        SemanticTokenType::NUMBER,
                        SemanticTokenType::REGEXP,
                        SemanticTokenType::OPERATOR,
                        SemanticTokenType::DECORATOR,
                    ],
                    token_modifiers: vec![
                        SemanticTokenModifier::DECLARATION,
                        SemanticTokenModifier::DEFINITION,
                        SemanticTokenModifier::READONLY,
                        SemanticTokenModifier::STATIC,
                        SemanticTokenModifier::DEPRECATED,
                        SemanticTokenModifier::ABSTRACT,
                        SemanticTokenModifier::ASYNC,
                        SemanticTokenModifier::MODIFICATION,
                        SemanticTokenModifier::DOCUMENTATION,
                        SemanticTokenModifier::DEFAULT_LIBRARY,
                    ],
                },
                range: None,
                full: Some(SemanticTokensFullOptions::Bool(true)),
            },
        )),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..Default::default()
    })
    .unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(connection, initialization_params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let pool = Builder::new_multi_thread().build().unwrap();
    let mut documents: HashMap<Url, String> = HashMap::new();
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    if params
        .capabilities
        .text_document
        .clone()
        .unwrap()
        .semantic_tokens
        .unwrap()
        .augments_syntax_tokens
        .unwrap()
    {
        panic!("Augmenting syntax tokens!")
    }

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                match cast::<SemanticTokensFullRequest>(req) {
                    Ok((id, params)) => {
                        pool.spawn(parse_semantic_tokens(
                            id,
                            documents[&params.text_document.uri].clone(),
                            connection.sender.clone(),
                        ));
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
            }
            Message::Response(_resp) => {}
            Message::Notification(not) => {
                let not = match cast_not::<DidOpenTextDocument>(not) {
                    Ok(params) => {
                        documents.insert(params.text_document.uri, params.text_document.text);
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
                let _not = match cast_not::<DidChangeTextDocument>(not) {
                    Ok(params) => {
                        // Assume it's only one thing being changed across the whole document
                        documents.insert(
                            params.text_document.uri,
                            params.content_changes[0].text.clone(),
                        );
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
            }
        }
    }
    Ok(())
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn cast_not<R>(req: Notification) -> Result<R::Params, ExtractError<Notification>>
where
    R: lsp_types::notification::Notification,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
