//! LSP server setup and core backend.
//!
//! The [`Backend`] struct holds shared state (the LSP client handle and vault
//! root path) and implements the [`LanguageServer`] trait by dispatching to
//! the feature-specific submodules.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, OneOf, Position,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::root;

use super::{completion, definition, diagnostics, hover, symbols};

/// Start the LSP server on stdin/stdout.
///
/// Discovers the vault root from `path` (or cwd), then enters the tower-lsp
/// event loop. This function blocks until the client disconnects.
pub async fn serve(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => path,
        None => std::env::current_dir().context("failed to read current directory")?,
    };

    let root = root::find_root(&start)?;
    let root_for_backend = root.clone();

    let (service, socket) =
        LspService::new(move |client| Backend::new(client, root_for_backend.clone()));
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;

    Ok(())
}

/// Shared state for the LSP server.
///
/// Each request handler receives a reference to this struct and uses `root`
/// to build the vault index and `client` to send notifications back to the editor.
pub(super) struct Backend {
    /// Handle for sending notifications and log messages to the editor.
    pub(super) client: Client,
    /// Canonical absolute path to the vault root directory.
    pub(super) root: PathBuf,
    /// In-memory text for currently open documents.
    ///
    /// We use this for completion/diagnostics so unsaved edits are reflected
    /// immediately instead of requiring a save.
    documents: RwLock<HashMap<Url, String>>,
}

impl Backend {
    pub(super) fn new(client: Client, root: PathBuf) -> Self {
        Self {
            client,
            root,
            documents: RwLock::new(HashMap::new()),
        }
    }

    /// Convert a document URI to its absolute and vault-relative paths.
    ///
    /// Returns `None` if the URI isn't a file, isn't under the vault root,
    /// or isn't a markdown file.
    pub(super) fn markdown_rel_path(&self, uri: &Url) -> Option<(PathBuf, PathBuf)> {
        let abs = uri.to_file_path().ok()?;
        if !abs.starts_with(&self.root) {
            return None;
        }

        let rel = abs.strip_prefix(&self.root).ok()?;
        let rel = crate::index::normalize_rel_path(rel)?;
        if !is_markdown_path(&rel) {
            return None;
        }

        Some((abs, rel))
    }

    /// Return current document text, preferring in-memory content for open files.
    pub(super) async fn document_text(&self, uri: &Url, abs_path: &Path) -> Option<String> {
        if let Some(text) = self.documents.read().await.get(uri).cloned() {
            return Some(text);
        }
        std::fs::read_to_string(abs_path).ok()
    }

    /// Update in-memory text for an open document.
    pub(super) async fn set_document_text(&self, uri: Url, text: String) {
        self.documents.write().await.insert(uri, text);
    }

    /// Remove in-memory text for a closed document.
    pub(super) async fn clear_document_text(&self, uri: &Url) {
        self.documents.write().await.remove(uri);
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: None,
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "[".to_string(),
                        "(".to_string(),
                        "#".to_string(),
                    ]),
                    ..CompletionOptions::default()
                }),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("kdb lsp connected at {}", self.root.display()),
            )
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.set_document_text(
            params.text_document.uri.clone(),
            params.text_document.text.clone(),
        )
        .await;
        diagnostics::did_open(self, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.last() {
            self.set_document_text(params.text_document.uri.clone(), change.text.clone())
                .await;
        }
        diagnostics::did_change(self, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.clear_document_text(&params.text_document.uri).await;
        diagnostics::did_close(self, params).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        symbols::document_symbol(self, params).await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        definition::goto_definition(self, params).await
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        completion::completion(self, params).await
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        hover::hover(self, params).await
    }
}

/// Check whether a path has a `.md` extension (case-insensitive).
pub(super) fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

/// Compute a relative path from `from_dir` to `to`.
///
/// Used to generate relative link paths for autocomplete suggestions,
/// so that completions are valid regardless of the source file's location.
pub(super) fn relative_path(from_dir: &Path, to: &Path) -> PathBuf {
    let from = from_dir
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();
    let to = to
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();

    let mut common = 0usize;
    while common < from.len() && common < to.len() && from[common] == to[common] {
        common += 1;
    }

    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for part in &to[common..] {
        out.push(part);
    }

    if out.as_os_str().is_empty() {
        to.iter().collect()
    } else {
        out
    }
}

/// Convert a path to a forward-slash string for use in markdown links.
pub(super) fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Convert an LSP position (line + UTF-16 character offset) to a byte offset.
///
/// LSP uses UTF-16 code units for character offsets, but Rust strings are
/// UTF-8, so this performs the necessary mapping.
pub(super) fn position_to_byte_offset(content: &str, position: Position) -> Option<usize> {
    let mut line_start = 0usize;
    for _ in 0..position.line {
        let rel_newline = content[line_start..].find('\n')?;
        line_start += rel_newline + 1;
    }

    let line_end = content[line_start..]
        .find('\n')
        .map_or(content.len(), |rel_newline| line_start + rel_newline);
    let line = &content[line_start..line_end];

    let mut utf16_col = 0u32;
    for (byte_offset, ch) in line.char_indices() {
        if utf16_col >= position.character {
            return Some(line_start + byte_offset);
        }
        utf16_col += ch.len_utf16() as u32;
    }

    Some(line_end)
}
