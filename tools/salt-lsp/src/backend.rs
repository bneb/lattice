//! Salt LSP Backend — LanguageServer trait implementation
//!
//! Zero-I/O architecture: salt-front is linked as a library crate.
//! On every keystroke, source text is passed directly to the compiler's
//! in-memory pipeline for <5ms diagnostic latency.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::completion;
use crate::diagnostics;
use crate::sir_index::SirIndex;

/// In-memory document and symbol state.
pub struct DocumentState {
    /// URI → full text content
    pub documents: HashMap<Url, String>,
    /// SIR symbol index (populated from in-memory compilation)
    pub sir_index: SirIndex,
}

pub struct SaltBackend {
    client: Client,
    state: Arc<RwLock<DocumentState>>,
}

impl SaltBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(DocumentState {
                documents: HashMap::new(),
                sir_index: SirIndex::new(),
            })),
        }
    }

    /// Run two-tier diagnostics and update the SIR index.
    async fn publish_diagnostics(&self, uri: Url, text: &str) {
        // Tier 1: Fast pattern-based diagnostics (instant)
        let mut diags = diagnostics::diagnose(text);

        // Tier 2: In-memory compiler diagnostics (<5ms via salt-front library)
        let module_name = uri.path_segments()
            .and_then(|s| s.last())
            .unwrap_or("unknown")
            .trim_end_matches(".salt")
            .to_string();

        let (compiler_diags, sir_module) = diagnostics::diagnose_with_compiler(text, &module_name);
        diags.extend(compiler_diags);

        // Update SIR index if compilation succeeded
        if let Some(module) = sir_module {
            let mut state = self.state.write().await;
            state.sir_index.update(uri.clone(), module);
        }

        self.client
            .publish_diagnostics(uri, diags, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SaltBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        ":".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "salt-lsp".to_string(),
                version: Some("0.2.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "Salt LSP v0.2.0 initialized — in-memory compilation active",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();

        {
            let mut state = self.state.write().await;
            state.documents.insert(uri.clone(), text.clone());
        }

        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        if let Some(change) = params.content_changes.into_iter().next() {
            let text = change.text;
            {
                let mut state = self.state.write().await;
                state.documents.insert(uri.clone(), text.clone());
            }
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut state = self.state.write().await;
        state.documents.remove(&params.text_document.uri);
        state.sir_index.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t.as_str(),
            None => return Ok(None),
        };

        let mut items = completion::complete(text, position);

        // SIR-powered function completions
        for name in state.sir_index.all_function_names() {
            if !items.iter().any(|i| i.label == name) {
                let detail = state.sir_index.lookup_function(name).map(|func| {
                    format!("fn {}({} params) -> {:?}", name, func.params.len(), func.return_type)
                });
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail,
                    ..Default::default()
                });
            }
        }

        // SIR-powered struct completions
        for name in state.sir_index.all_struct_names() {
            if !items.iter().any(|i| i.label == name) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some("struct".to_string()),
                    ..Default::default()
                });
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t.as_str(),
            None => return Ok(None),
        };

        let word = extract_word_at(text, position);

        // Priority 1: SIR function hover (signature + contracts)
        if let Some(func) = state.sir_index.lookup_function(&word) {
            let hover_text = SirIndex::format_function_hover(func);
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: None,
            }));
        }

        // Priority 2: SIR struct hover (field layout)
        if let Some(s) = state.sir_index.lookup_struct(&word) {
            let hover_text = SirIndex::format_struct_hover(s);
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: None,
            }));
        }

        // Priority 3: Keyword/builtin type info
        if let Some(info) = completion::keyword_info(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info.to_string(),
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t.as_str(),
            None => return Ok(None),
        };

        let word = extract_word_at(text, position);
        if word.is_empty() {
            return Ok(None);
        }

        if let Some(location) = state.sir_index.find_definition(&word) {
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }

        Ok(None)
    }
}

/// Extract the word at the given cursor position.
fn extract_word_at(text: &str, position: Position) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return String::new();
    }
    let line = lines[line_idx];
    let col = position.character as usize;
    if col > line.len() {
        return String::new();
    }

    let bytes = line.as_bytes();
    let mut start = col;
    let mut end = col;

    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }

    line[start..end].to_string()
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
