use std::error::Error;

use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};
use lsp_types::{
    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, OneOf, ServerCapabilities, Location, Range, Position,
};

// This is mostly based off https://github.com/rust-lang/rust-analyzer/blob/master/lib/lsp-server/examples/goto_def.rs
fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    eprintln!("Starting LSP server.");

    let (connection, io_threads) = Connection::listen("127.0.0.1:5007")?;

    eprintln!("Got connection.");

    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
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

    eprintln!("Shutting down server.");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
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
                match cast::<GotoDefinition>(req) {
                    Ok((id, params)) => {
                        eprintln!("Go gotoDefinition request #{id}: {params:?}");
                        let uri = params.text_document_position_params.text_document.uri;

                        let result = Some(GotoDefinitionResponse::Scalar(Location::new(uri, Range::new(Position::new(1, 1), Position::new(1,1)))));
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
