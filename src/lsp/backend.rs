//! LSP server setup and core backend.
//!
//! The [`Backend`] struct holds shared state (the LSP client handle and vault
//! root path) and implements the [`LanguageServer`] trait by dispatching to
//! the feature-specific submodules.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use tokio::task;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWatchedFilesRegistrationOptions,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    DocumentSymbolParams, DocumentSymbolResponse, FileChangeType, FileSystemWatcher, GlobPattern,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams,
    InitializeResult, InitializedParams, MessageType, OneOf, Position, Registration,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextEdit, Url, WatchKind,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::index::VaultIndex;
use crate::lang::CodeLanguage;
use crate::project;

use super::{completion, definition, diagnostics, formatting, hover, symbols};

// --------------------------------------------------------
// kdb/src/lsp/backend.rs
//
// pub async fn serve()                                 L75
// pub(super) struct Backend                            L97
//   pub(super) fn new()                               L114
//   fn set_watched_files_support()                    L124
//   async fn build_index()                            L129
//   async fn ensure_index_loaded()                    L137
//   pub(super) async fn ensure_index()                L151
//   pub(super) async fn with_index()                  L166
//   pub(super) fn markdown_rel_path()                 L178
//   pub(super) fn code_rel_path()                     L194
//   pub(super) async fn document_text()               L210
//   pub(super) async fn set_document_text()           L218
//   pub(super) async fn clear_document_text()         L223
//   pub(super) async fn open_document_uris()          L228
//   pub(super) async fn sync_document_into_index()    L241
//   pub(super) async fn sync_document_from_disk()     L256
//   async fn register_markdown_watcher()              L270
//   async fn sync_watched_files_into_index()          L310
//   async fn initialize()                             L359
//   async fn initialized()                            L398
//   async fn shutdown()                               L409
//   async fn did_open()                               L413
//   async fn did_change()                             L424
//   async fn did_close()                              L434
//   async fn did_change_watched_files()               L441
//   async fn document_symbol()                        L447
//   async fn goto_definition()                        L454
//   async fn completion()                             L461
//   async fn hover()                                  L465
//   async fn formatting()                             L469
// pub(super) fn is_markdown_path()                    L478
// pub(super) fn relative_path()                       L488
// pub(super) fn path_to_slash()                       L519
// pub(super) fn position_to_byte_offset()             L527
// --------------------------------------------------------

/// Start the LSP server on stdin/stdout.
///
/// Discovers the vault root from `path` (or cwd), then enters the tower-lsp
/// event loop. This function blocks until the client disconnects.
pub async fn serve(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => path,
        None => std::env::current_dir().context("failed to read current directory")?,
    };

    let root = project::root::find_root(&start)?;
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
/// Each request handler receives a reference to this struct. The vault index
/// is built once and cached, then incrementally updated from document events.
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
    /// Cached vault index shared by all requests.
    index: RwLock<Option<VaultIndex>>,
    /// Whether the client supports dynamic watched-files registration.
    supports_dynamic_watched_files: AtomicBool,
}

impl Backend {
    pub(super) fn new(client: Client, root: PathBuf) -> Self {
        Self {
            client,
            root,
            documents: RwLock::new(HashMap::new()),
            index: RwLock::new(None),
            supports_dynamic_watched_files: AtomicBool::new(false),
        }
    }

    fn set_watched_files_support(&self, supported: bool) {
        self.supports_dynamic_watched_files
            .store(supported, Ordering::Relaxed);
    }

    async fn build_index(&self) -> Result<VaultIndex> {
        let root = self.root.clone();
        let ignore_patterns = project::config::load_index_ignores(&root)?;
        task::spawn_blocking(move || VaultIndex::build_with_ignores(&root, &ignore_patterns))
            .await
            .context("failed to join vault index build task")?
    }

    async fn ensure_index_loaded(&self) -> Result<()> {
        if self.index.read().await.is_some() {
            return Ok(());
        }

        let index = self.build_index().await?;
        let mut guard = self.index.write().await;
        if guard.is_none() {
            *guard = Some(index);
        }
        Ok(())
    }

    /// Ensure the cached index exists, logging and returning false on failure.
    pub(super) async fn ensure_index(&self) -> bool {
        if let Err(error) = self.ensure_index_loaded().await {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("failed to build vault index: {error:#}"),
                )
                .await;
            return false;
        }

        true
    }

    /// Read from the cached index if available.
    pub(super) async fn with_index<T, F>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&VaultIndex) -> T,
    {
        let guard = self.index.read().await;
        guard.as_ref().map(f)
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
        let rel = crate::project::paths::normalize_rel_path(rel)?;
        if !is_markdown_path(&rel) {
            return None;
        }

        Some((abs, rel))
    }

    /// Convert a document URI to absolute and relative paths for supported code files.
    pub(super) fn code_rel_path(&self, uri: &Url) -> Option<(PathBuf, PathBuf)> {
        let abs = uri.to_file_path().ok()?;
        if !abs.starts_with(&self.root) {
            return None;
        }

        let rel = abs.strip_prefix(&self.root).ok()?;
        let rel = crate::project::paths::normalize_rel_path(rel)?;
        if CodeLanguage::from_path(&rel).is_none() {
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

    /// Snapshot of all currently open markdown document URIs.
    pub(super) async fn open_document_uris(&self) -> Vec<Url> {
        let mut uris = self
            .documents
            .read()
            .await
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        uris.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        uris
    }

    /// Apply in-memory document content to the cached index.
    pub(super) async fn sync_document_into_index(&self, uri: &Url, content: &str) {
        let Some((abs_path, rel_path)) = self.markdown_rel_path(uri) else {
            return;
        };
        if !self.ensure_index().await {
            return;
        }

        let mut guard = self.index.write().await;
        if let Some(index) = guard.as_mut() {
            index.upsert_file(rel_path, abs_path, content);
        }
    }

    /// Re-sync a document from disk after the editor closes it.
    pub(super) async fn sync_document_from_disk(&self, uri: &Url) {
        let Some((_, rel_path)) = self.markdown_rel_path(uri) else {
            return;
        };
        if !self.ensure_index().await {
            return;
        }

        let mut guard = self.index.write().await;
        if let Some(index) = guard.as_mut() {
            index.reload_file(&rel_path);
        }
    }

    async fn register_markdown_watcher(&self) {
        if !self.supports_dynamic_watched_files.load(Ordering::Relaxed) {
            return;
        }

        let register_options =
            match serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                watchers: vec![FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.md".to_string()),
                    kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                }],
            }) {
                Ok(value) => value,
                Err(error) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!("failed to serialize watched-files registration: {error:#}"),
                        )
                        .await;
                    return;
                }
            };

        let registration = Registration {
            id: "kdb-watch-markdown".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(register_options),
        };

        if let Err(error) = self.client.register_capability(vec![registration]).await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("failed to register markdown file watcher: {error:#}"),
                )
                .await;
        }
    }

    async fn sync_watched_files_into_index(&self, params: &DidChangeWatchedFilesParams) -> bool {
        let updates = params
            .changes
            .iter()
            .filter_map(|change| {
                self.markdown_rel_path(&change.uri)
                    .map(|(abs_path, rel_path)| {
                        (change.typ, change.uri.clone(), abs_path, rel_path)
                    })
            })
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return false;
        }
        if !self.ensure_index().await {
            return false;
        }

        let open_documents = self.documents.read().await.clone();
        let mut changed = false;

        let mut guard = self.index.write().await;
        let Some(index) = guard.as_mut() else {
            return false;
        };

        for (change_type, uri, abs_path, rel_path) in updates {
            if change_type == FileChangeType::DELETED {
                index.remove_file(&rel_path);
                changed = true;
                continue;
            }

            if change_type == FileChangeType::CREATED || change_type == FileChangeType::CHANGED {
                if let Some(content) = open_documents.get(&uri) {
                    index.upsert_file(rel_path, abs_path, content);
                } else {
                    index.reload_file(&rel_path);
                }
                changed = true;
            }
        }

        changed
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let supports_dynamic_watched_files = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files)
            .and_then(|caps| caps.dynamic_registration)
            .unwrap_or(false);
        self.set_watched_files_support(supports_dynamic_watched_files);

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
                document_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.register_markdown_watcher().await;
        let _ = self.ensure_index().await;
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
        self.sync_document_into_index(&params.text_document.uri, &params.text_document.text)
            .await;
        diagnostics::did_open(self, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.last() {
            self.set_document_text(params.text_document.uri.clone(), change.text.clone())
                .await;
            self.sync_document_into_index(&params.text_document.uri, &change.text)
                .await;
        }
        diagnostics::did_change(self, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.clear_document_text(&params.text_document.uri).await;
        self.sync_document_from_disk(&params.text_document.uri)
            .await;
        diagnostics::did_close(self, params).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        if self.sync_watched_files_into_index(&params).await {
            diagnostics::refresh_open_documents(self).await;
        }
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

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        formatting::formatting(self, params).await
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
