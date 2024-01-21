use std::{collections::HashMap, error::Error};

use abasic_core::{DiagnosticMessage, SourceFileAnalyzer, TokenType};
use lsp_server::{
    Connection, ErrorCode, ExtractError, Message, Notification as ServerNotification,
    Request as ServerRequest, RequestId, Response, ResponseError,
};
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument, PublishDiagnostics},
    request::SemanticTokensFullRequest,
    Diagnostic, DiagnosticSeverity, InitializeParams, Position, PublishDiagnosticsParams, Range,
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, WorkDoneProgressOptions,
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

const TOKEN_TYPES: &[SemanticTokenType; 8] = &[
    SemanticTokenType::VARIABLE, // 0
    SemanticTokenType::STRING,   // 1
    SemanticTokenType::NUMBER,   // 2
    SemanticTokenType::OPERATOR, // 3
    SemanticTokenType::COMMENT,  // 4
    SemanticTokenType::KEYWORD,  // 5
    SemanticTokenType::MODIFIER, // 6
    SemanticTokenType::REGEXP,   // 7
];

fn abasic_token_type_to_lsp_token_type(abasic_token_type: TokenType) -> u32 {
    match abasic_token_type {
        // These numbers are indices into `TOKEN_TYPES`.
        TokenType::Symbol => 0,
        TokenType::String => 1,
        TokenType::Number => 2,
        TokenType::Operator => 3,
        TokenType::Comment => 4,
        TokenType::Keyword => 5,
        TokenType::Delimiter => 6,
        TokenType::Data => 7,
    }
}

fn handle_one_connection() -> LspResult<()> {
    eprintln!("Waiting for connection.");

    let (connection, io_threads) = Connection::listen(LISTEN_ADDR)?;

    eprintln!("Got connection.");

    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        semantic_tokens_provider: Some(
            lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(
                SemanticTokensOptions {
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    legend: SemanticTokensLegend {
                        token_types: Vec::from(TOKEN_TYPES),
                        token_modifiers: vec![],
                    },
                    range: None,
                    // TODO: It'd be nice to be more incremental in our approach,
                    // it's expensive to send a full re-tokenization on every
                    // keystroke.
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                },
            ),
        ),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                // TODO: It'd be nice to do deltas eventually, this can get really
                // expensive to do on every keystroke.
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

    let mut files: HashMap<String, SourceFileAnalyzer> = HashMap::new();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                eprintln!("Got request: {}", req.method);
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                let req = match cast_request::<SemanticTokensFullRequest>(req) {
                    CastResult::Match((id, params)) => {
                        let Some(analyzer) = files.get(&params.text_document.uri.to_string())
                        else {
                            connection.sender.send(Message::Response(Response {
                                id,
                                result: None,
                                error: Some(ResponseError {
                                    code: ErrorCode::RequestFailed as i32,
                                    message: "File contents have not been sent by client"
                                        .to_string(),
                                    data: None,
                                }),
                            }))?;
                            continue;
                        };

                        let mut data: Vec<SemanticToken> = vec![];
                        let mut prev_line_number = 0;
                        for (line_number, line) in analyzer.token_types().iter().enumerate() {
                            let mut prev_token_start = 0;
                            for (abasic_token_type, range) in line {
                                let delta_line = (line_number - prev_line_number) as u32;
                                prev_line_number = line_number;
                                let delta_start = (range.start - prev_token_start) as u32;
                                prev_token_start = range.start;
                                let length = range.len() as u32;
                                let token_type =
                                    abasic_token_type_to_lsp_token_type(*abasic_token_type);
                                data.push(SemanticToken {
                                    delta_line,
                                    delta_start,
                                    length,
                                    token_type,
                                    token_modifiers_bitset: 0,
                                })
                            }
                        }

                        let result = Some(SemanticTokens {
                            result_id: None,
                            data,
                        });
                        let result = serde_json::to_value(&result).unwrap();
                        connection.sender.send(Message::Response(Response {
                            id,
                            result: Some(result),
                            error: None,
                        }))?;
                        continue;
                    }
                    CastResult::NoMatch(req) => req,
                };
                eprintln!("Unhandled request: {req:?}");
            }
            Message::Response(resp) => {
                eprintln!("Unhandled response: {resp:?}");
            }
            Message::Notification(not) => {
                eprintln!("Got notification: {}", not.method);
                let not = match cast_notification::<DidOpenTextDocument>(not) {
                    CastResult::Match(params) => {
                        let analyzer = SourceFileAnalyzer::analyze(params.text_document.text);
                        let diagnostics = analyze_source_file(&analyzer);
                        files.insert(params.text_document.uri.to_string(), analyzer);
                        send_notification::<PublishDiagnostics>(
                            &connection,
                            PublishDiagnosticsParams {
                                uri: params.text_document.uri,
                                diagnostics,
                                version: None,
                            },
                        )?;
                        continue;
                    }
                    CastResult::NoMatch(not) => not,
                };
                let not = match cast_notification::<DidChangeTextDocument>(not) {
                    CastResult::Match(params) => {
                        // TODO: I think we only get one change b/c we're using TextDocumentSyncKind::FULL but not sure...
                        if let Some(last_change) = params.content_changes.into_iter().last() {
                            let analyzer = SourceFileAnalyzer::analyze(last_change.text);
                            let diagnostics = analyze_source_file(&analyzer);
                            files.insert(params.text_document.uri.to_string(), analyzer);
                            send_notification::<PublishDiagnostics>(
                                &connection,
                                PublishDiagnosticsParams {
                                    uri: params.text_document.uri,
                                    diagnostics,
                                    version: None,
                                },
                            )?;
                        }
                        continue;
                    }
                    CastResult::NoMatch(not) => not,
                };
                let not = match cast_notification::<DidChangeTextDocument>(not) {
                    CastResult::Match(params) => {
                        files.remove(&params.text_document.uri.to_string());
                        continue;
                    }
                    CastResult::NoMatch(not) => not,
                };
                eprintln!("Unhandled notification: {not:?}");
            }
        }
    }

    Ok(())
}

fn analyze_source_file(analyzer: &SourceFileAnalyzer) -> Vec<Diagnostic> {
    let messages = analyzer.messages();
    let mut diagnostics: Vec<Diagnostic> = vec![];
    let source_map = analyzer.source_file_map();
    for message in messages {
        if let Some((line, range)) = source_map.map_to_source(&message) {
            let diag_range = Range::new(
                Position::new(line as u32, range.start as u32),
                Position::new(line as u32, range.end as u32),
            );
            let (severity, content) = match message {
                DiagnosticMessage::Warning(_line, msg) => {
                    (DiagnosticSeverity::WARNING, msg.clone())
                }
                DiagnosticMessage::Error(_line, err) => {
                    (DiagnosticSeverity::ERROR, err.to_string())
                }
            };
            let mut diag = Diagnostic::new_simple(diag_range, content);
            diag.severity = Some(severity);
            diagnostics.push(diag);
        }
    }
    diagnostics
}

/// Represents the result of an attempted cast. In the case of no match, ownership of
/// the original subject is passed back to the caller so they can do something with it
/// (e.g., attempt to cast it to something else).
pub enum CastResult<Subject, Target> {
    Match(Target),
    NoMatch(Subject),
}

fn cast_request<R>(req: ServerRequest) -> CastResult<ServerRequest, (RequestId, R::Params)>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    match req.extract::<R::Params>(R::METHOD) {
        Ok(result) => CastResult::Match(result),
        Err(ExtractError::MethodMismatch(req)) => CastResult::NoMatch(req),
        Err(ExtractError::JsonError { method, error }) => {
            panic!("Failed to deserialize {method} {error:?}");
        }
    }
}

fn cast_notification<N>(not: ServerNotification) -> CastResult<ServerNotification, N::Params>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    match not.extract::<N::Params>(N::METHOD) {
        Ok(params) => CastResult::Match(params),
        Err(ExtractError::MethodMismatch(not)) => CastResult::NoMatch(not),
        Err(ExtractError::JsonError { method, error }) => {
            panic!("Failed to deserialize {method} {error:?}");
        }
    }
}

fn send_notification<N: lsp_types::notification::Notification>(
    connection: &Connection,
    params: N::Params,
) -> LspResult<()> {
    let not = ServerNotification::new(N::METHOD.to_string(), params);
    connection.sender.send(not.into())?;
    Ok(())
}
