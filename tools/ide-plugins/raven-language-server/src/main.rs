use std::error::Error;

use lsp_server::{Connection, ExtractError, Message, Request, RequestId};
use lsp_types::{InitializeParams, PositionEncodingKind, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities, SemanticTokenType, ServerCapabilities};
use lsp_types::request::SemanticTokensFullRequest;
use tokio::runtime::Builder;

use crate::semantic_tokens::parse_semantic_tokens;

mod semantic_tokens;

pub fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
            work_done_progress_options: Default::default(),
            legend: SemanticTokensLegend {
                token_types: vec!(SemanticTokenType::CLASS, SemanticTokenType::COMMENT),
                token_modifiers: vec!(),
            },
            range: None,
            full: Some(SemanticTokensFullOptions::Bool(true)),
        })),
        ..Default::default()
    }).unwrap();
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
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    if params.capabilities.text_document.unwrap().semantic_tokens.unwrap().augments_syntax_tokens.unwrap() {
        panic!("Augmenting syntax tokens!")
    }

    for msg in &connection.receiver {
        eprintln!("got msg: {msg:?}");
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("got request: {req:?}");
                match cast::<SemanticTokensFullRequest>(req) {
                    Ok((id, params)) => {
                        pool.spawn(parse_semantic_tokens(id, params.text_document.uri, connection.sender.clone()));
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
                // ...
            }
            Message::Response(resp) => {
                eprintln!("got response: {resp:?}");
            }
            Message::Notification(not) => {
                eprintln!("got notification: {not:?}");
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