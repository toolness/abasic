use std::error::Error;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    notification::DidOpenTextDocument, request::GotoDefinition, GotoDefinitionResponse,
    InitializeParams, Location, OneOf, Position, Range, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
};

type LspResult<T> = Result<T, Box<dyn Error + Sync + Send>>;

const LISTEN_ADDR: &'static str = "127.0.0.1:5007";

// This is mostly based off https://github.com/rust-lang/rust-analyzer/blob/master/lib/lsp-server/examples/goto_def.rs
fn main() -> LspResult<()> {
    eprintln!("Starting LSP server on {LISTEN_ADDR}.");

    loop {
        handle_one_connection()?;
    }
}

fn handle_one_connection() -> LspResult<()> {
    eprintln!("Waiting for connection.");

    let (connection, io_threads) = Connection::listen(LISTEN_ADDR)?;

    eprintln!("Got connection.");

    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                will_save: None,
                will_save_wait_until: None,
                save: None,
            },
        )),
        ..Default::default()
    })
    .unwrap();

    let initialization_params = match connection.initialize(server_capabilities) {
        Ok(it) => it,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join()?;
            }
            return Err(e.into());
        }
    };
    main_loop(connection, initialization_params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: Connection, params: serde_json::Value) -> LspResult<()> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    eprintln!("Starting main loop.");

    for msg in &connection.receiver {
        eprintln!("Got message: {msg:?}");
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("Got request: {req:?}");
                match cast_request::<GotoDefinition>(req) {
                    Ok((id, params)) => {
                        eprintln!("Go gotoDefinition request #{id}: {params:?}");
                        let uri = params.text_document_position_params.text_document.uri;

                        let result = Some(GotoDefinitionResponse::Scalar(Location::new(
                            uri,
                            Range::new(Position::new(1, 1), Position::new(1, 1)),
                        )));
                        let result = serde_json::to_value(&result).unwrap();
                        let resp = Response {
                            id,
                            result: Some(result),
                            error: None,
                        };
                        connection.sender.send(Message::Response(resp))?;
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
                // ...
            }
            Message::Response(resp) => {
                eprintln!("Got response: {resp:?}");
            }
            Message::Notification(not) => {
                eprintln!("Got notification: {not:?}");
                match cast_notification::<DidOpenTextDocument>(not) {
                    Ok(params) => {
                        let text = params.text_document.text;
                        println!("Document text: {text}");
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(not)) => not,
                };
            }
        }
    }

    Ok(())
}

fn cast_request<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn cast_notification<N>(not: Notification) -> Result<N::Params, ExtractError<Notification>>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    not.extract(N::METHOD)
}
