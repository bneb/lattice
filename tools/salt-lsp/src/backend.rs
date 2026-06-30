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
use crate::semantic_tokens;
use crate::source_check;
use crate::sir_index::{SirIndex, SymbolKind as SirSymbolKind};

pub struct DocumentState {
    pub documents: HashMap<Url, String>,
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

    async fn publish_diagnostics(&self, uri: Url, text: &str) {
        let mut diags = diagnostics::diagnose(text);

        let module_name = uri.path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or("unknown")
            .trim_end_matches(".salt")
            .to_string();

        let (compiler_diags, sir_module) = diagnostics::diagnose_with_compiler(text, &module_name);
        diags.extend(compiler_diags);

        if let Some(module) = sir_module {
            let mut state = self.state.write().await;
            state.sir_index.update(uri.clone(), module);
        }

        self.client.publish_diagnostics(uri, diags, None).await;
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
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                // ── New capabilities ──────────────────────────────────
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                            legend: semantic_tokens::legend(),
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    )
                ),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "salt-lsp".to_string(),
                version: Some("0.3.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "Salt LSP v0.3.0 — semantic tokens, references, document symbols, code actions",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }

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

        if let Some(func) = state.sir_index.lookup_function(&word) {
            let hover_text = SirIndex::format_function_hover(func);
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown, value: hover_text,
                }),
                range: None,
            }));
        }
        if let Some(s) = state.sir_index.lookup_struct(&word) {
            let hover_text = SirIndex::format_struct_hover(s);
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown, value: hover_text,
                }),
                range: None,
            }));
        }
        if let Some(info) = completion::keyword_info(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown, value: info.to_string(),
                }),
                range: None,
            }));
        }
        Ok(None)
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t.as_str(),
            None => return Ok(None),
        };
        let word = extract_word_at(text, position);
        if word.is_empty() { return Ok(None); }
        if let Some(location) = state.sir_index.find_definition(&word) {
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }
        // Check local definitions (let bindings, params, struct fields)
        if let Some((line, col)) = source_check::find_var_definition(text, &word, position.line as usize) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position { line: line as u32, character: col as u32 },
                    end: Position { line: line as u32, character: (col + word.len()) as u32 },
                },
            })));
        }
        Ok(None)
    }

    // ── References (new) ────────────────────────────────────────────

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t.as_str(),
            None => return Ok(None),
        };
        let word = extract_word_at(text, position);
        if word.is_empty() { return Ok(None); }

        let refs = state.sir_index.find_references(&word);
        if refs.is_empty() {
            // If no type-based refs found, try definition as fallback
            if let Some(def) = state.sir_index.find_definition(&word) {
                return Ok(Some(vec![def]));
            }
            return Ok(None);
        }
        Ok(Some(refs))
    }

    // ── Document Symbols (new) ──────────────────────────────────────

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let state = self.state.read().await;

        let entries = state.sir_index.document_symbols_for(uri);
        if entries.is_empty() {
            return Ok(None);
        }

        let symbols: Vec<DocumentSymbol> = entries.iter().map(|e| {
            let kind = match e.kind {
                SirSymbolKind::FUNCTION => SymbolKind::FUNCTION,
                SirSymbolKind::STRUCT => SymbolKind::STRUCT,
            };
            DocumentSymbol {
                name: if e.is_pub {
                    format!("pub {}", e.name)
                } else {
                    e.name.clone()
                },
                detail: Some(e.detail.clone()),
                kind,
                range: Range {
                    start: Position::new(e.line, e.column),
                    end: Position::new(e.line, e.column + e.name.len() as u32),
                },
                selection_range: Range {
                    start: Position::new(e.line, e.column),
                    end: Position::new(e.line, e.column + e.name.len() as u32),
                },
                children: None,
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
            }
        }).collect();

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    // ── Semantic Tokens (new) ───────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t.as_str(),
            None => return Ok(None),
        };

        let tokens = semantic_tokens::tokenize(text);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    // ── Code Actions (new) ──────────────────────────────────────────

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let diags = params.context.diagnostics;

        let mut actions = Vec::new();

        for diag in &diags {
            // Offer to wrap in unsafe if "unsafe operation" diagnostic
            if diag.message.contains("VERIFICATION ERROR")
                || diag.message.contains("contract violation")
            {
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: "Add @trusted annotation to skip Z3 verification".to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some({
                            let mut map = HashMap::new();
                            map.insert(uri.clone(), vec![TextEdit {
                                range: Range {
                                    start: Position::new(diag.range.start.line, 0),
                                    end: Position::new(diag.range.start.line, 0),
                                },
                                new_text: "@trusted\n".to_string(),
                            }]);
                            map
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }));
            }

            // Offer to add requires clause for bounds violations
            if diag.message.contains("could not prove")
                || diag.message.contains("counterexample")
            {
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: "Add requires clause with the suggested constraint".to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some({
                            let mut map = HashMap::new();
                            map.insert(uri.clone(), vec![TextEdit {
                                range: Range {
                                    start: Position::new(diag.range.start.line, 0),
                                    end: Position::new(diag.range.start.line, 0),
                                },
                                new_text: "// add requires() clause based on the counterexample above\n".to_string(),
                            }]);
                            map
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }));
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}

/// Extract the word at the given cursor position.
fn extract_word_at(text: &str, position: Position) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() { return String::new(); }
    let line = lines[line_idx];
    let col = position.character as usize;
    if col > line.len() { return String::new(); }

    let bytes = line.as_bytes();
    let mut start = col;
    let mut end = col;
    while start > 0 && is_ident_char(bytes[start - 1]) { start -= 1; }
    while end < bytes.len() && is_ident_char(bytes[end]) { end += 1; }
    line[start..end].to_string()
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
