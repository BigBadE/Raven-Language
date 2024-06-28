use std::collections::HashMap;
use std::error::Error;
use std::fs;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId};
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument};
use lsp_types::request::{GotoDeclaration, SemanticTokensFullRequest};
use lsp_types::{
    DeclarationCapability, InitializeParams, SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url, WorkDoneProgressOptions,
};
use tokio::runtime::Builder;

use crate::semantic_tokens::{parse_semantic_tokens, TokenIterator};
use crate::syntax_manager::SyntaxManager;

/// This file is templated from Rust's LSP example.
mod semantic_tokens;
mod syntax_manager;

/// The main function, which sets up the server and starts the main loop
pub fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        declaration_provider: Some(DeclarationCapability::Simple(true)),
        // Semantic tokens provider gives the coloring of tokens
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
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
        })),
        // Text document sync synchronizes the documents between the LSP and the IDE
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..Default::default()
    })
    .unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(connection, initialization_params)?;
    // Wait for everything to finish before ending
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: Connection, params: serde_json::Value) -> Result<(), Box<dyn Error + Sync + Send>> {
    let pool = Builder::new_multi_thread().build().unwrap();
    let mut documents: HashMap<Url, String> = HashMap::new();
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    let mut syntax = SyntaxManager::default();

    // If augments_syntax_tokens is true, the IDE screws up handling semantic tokens
    if params.capabilities.text_document.clone().unwrap().semantic_tokens.unwrap().augments_syntax_tokens.unwrap() {
        panic!("Augmenting syntax tokens! Incompatible IDE!")
    }

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

                // Parse semantic tokens on another thread
                let req = match cast::<SemanticTokensFullRequest>(req) {
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
                match cast::<GotoDeclaration>(req) {
                    Ok((_id, params)) => {
                        syntax.get_syntax(params.text_document_position_params.text_document.uri.to_file_path().unwrap());
                        let mut position = params.text_document_position_params.position;
                        position.line += 1;
                        let token = TokenIterator::new(
                            fs::read(params.text_document_position_params.text_document.uri.to_file_path().unwrap())
                                .unwrap()
                                .as_slice(),
                        )
                        .find(|token| {
                            token.start.0 <= position.line
                                && token.end.0 >= position.line
                                && token.start.1 <= position.character
                                && token.end.1 >= position.character
                        });
                        if token.is_none() {
                            panic!("Failed to find token, server file view must be incorrect. Crashing to re-sync.");
                        }
                        let token = token.unwrap();
                        panic!(
                            "Found token {}",
                            token.to_string(
                                fs::read(params.text_document_position_params.text_document.uri.to_file_path().unwrap())
                                    .unwrap()
                                    .as_slice()
                            )
                        );
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
            }
            Message::Response(_resp) => {}
            Message::Notification(not) => {
                // Syncing is done on the main thread
                let not = match cast_not::<DidOpenTextDocument>(not) {
                    Ok(params) => {
                        if params.text_document.text.contains("ending") {
                            panic!("Ending");
                        }
                        documents.insert(params.text_document.uri, params.text_document.text);
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
                let _not = match cast_not::<DidChangeTextDocument>(not) {
                    Ok(params) => {
                        if params.content_changes[0].text.contains("ending") {
                            panic!("Ending");
                        }
                        // Assume it's only one thing being changed across the whole document
                        documents.insert(params.text_document.uri, params.content_changes[0].text.clone());
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

/// Tries to cast a general request into a single request
fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

/// Tries to cast a general notification into a single notification
fn cast_not<R>(req: Notification) -> Result<R::Params, ExtractError<Notification>>
where
    R: lsp_types::notification::Notification,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
