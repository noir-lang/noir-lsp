use std::cell::RefCell;
use std::collections::HashMap;

use dashmap::DashMap;
use noirc_frontend::{parse_program, ExpressionKind, ParsedModule, Statement};
use ropey::Rope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thread_safe::ThreadSafe;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
#[derive(Debug)]
struct Backend {
    client: Client,
    ast_map: DashMap<String, ThreadSafe<RefCell<ParsedModule>>>,
    document_map: DashMap<String, Rope>,
    // semantic_token_map: DashMap<String, Vec<ImCompleteSemanticToken>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            offset_encoding: None,
            capabilities: ServerCapabilities {
                inlay_hint_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
        })
        .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
        })
        .await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }
    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }

    async fn inlay_hint(
        &self,
        params: tower_lsp::lsp_types::InlayHintParams,
    ) -> Result<Option<Vec<InlayHint>>> {
        let mut inlays: Vec<(u32, u32, Option<String>)> = Vec::new();
        self.client
            .log_message(MessageType::INFO, "inlay hint")
            .await;
        let uri = &params.text_document.uri;
        self.client
            .log_message(MessageType::INFO, format!("{:?}", uri))
            .await;

        (*self
            .ast_map
            .get(uri.as_str())
            .unwrap()
            .value()
            .try_get_ref()
            .unwrap()
            .borrow())
        .functions
        .iter()
        .for_each(|func| match func.kind {
            noirc_frontend::FunctionKind::Normal => {
                func.def
                    .body
                    .0
                    .iter()
                    .for_each(|statement| match statement {
                        Statement::Let(val) => {
                            let literal_type = match &val.expression.kind {
                                ExpressionKind::Literal(lit) => match lit {
                                    noirc_frontend::Literal::Array(_) => {
                                        Some(": array".to_string())
                                    }
                                    noirc_frontend::Literal::Bool(_) => Some(": bool".to_string()),
                                    noirc_frontend::Literal::Integer(_) => {
                                        Some(": integer".to_string())
                                    }
                                    noirc_frontend::Literal::Str(_) => Some(": string".to_string()),
                                },
                                _ => None,
                            };
                            inlays.push((
                                val.expression.span.start(),
                                val.expression.span.end(),
                                literal_type,
                            ));
                        }
                        _ => {}
                    })
            }
            _ => {}
        });

        self.client
            .log_message(MessageType::INFO, format!("{:?}", inlays))
            .await;

        let document = match self.document_map.get(uri.as_str()) {
            Some(rope) => rope,
            None => return Ok(None),
        };

        let inlay_hint_list = inlays
            .iter()
            .map(|item| {
                // let start_position = offset_to_position(item.0, document)?;
                let end_position = offset_to_position((item.0 - 2) as usize, &document).unwrap();
                let inlay_hint = InlayHint {
                    text_edits: None,
                    tooltip: None,
                    kind: Some(InlayHintKind::TYPE),
                    padding_left: None,
                    padding_right: None,
                    data: None,
                    position: end_position,
                    label: InlayHintLabel::LabelParts(vec![InlayHintLabelPart {
                        value: item.2.clone().unwrap(),
                        tooltip: None,
                        location: Some(Location {
                            uri: params.text_document.uri.clone(),
                            range: Range {
                                start: Position::new(0, 4),
                                end: Position::new(0, 5),
                            },
                        }),
                        command: None,
                    }]),
                };
                inlay_hint
            })
            .collect::<Vec<_>>();

        Ok(Some(inlay_hint_list))
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
            .await;

        match self.client.apply_edit(WorkspaceEdit::default()).await {
            Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
            Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
            Err(err) => self.client.log_message(MessageType::ERROR, err).await,
        }

        Ok(None)
    }
}
#[derive(Debug, Deserialize, Serialize)]
struct InlayHintParams {
    path: String,
}

enum CustomNotification {}
impl Notification for CustomNotification {
    type Params = InlayHintParams;
    const METHOD: &'static str = "custom/notification";
}
struct TextDocumentItem {
    uri: Url,
    text: String,
    version: i32,
}
impl Backend {
    async fn on_change(&self, params: TextDocumentItem) {
        let rope = ropey::Rope::from_str(&params.text);
        self.document_map
            .insert(params.uri.to_string(), rope.clone());

        self.ast_map.insert(
            params.uri.to_string(),
            ThreadSafe::new(RefCell::new(parse_program(&params.text).0)),
        );
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        ast_map: DashMap::new(),
        document_map: DashMap::new(),
        // semantic_token_map: DashMap::new(),
    })
    .finish();

    serde_json::json!({"test": 20});
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    let line = rope.try_char_to_line(offset).ok()?;
    let first_char_of_line = rope.try_line_to_char(line).ok()?;
    let column = offset - first_char_of_line;
    Some(Position::new(line as u32, column as u32))
}
