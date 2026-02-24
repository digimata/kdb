//! Diagnostics publishing for broken links.
//!
//! On open/change we parse the current buffer text, resolve links against the
//! current vault index, and publish link errors as LSP diagnostics.

use std::path::Path;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, Position, Range, Url,
};

use crate::index::{
    Link, ParsedDocument, VaultIndex, parse_markdown, resolve_target_path, slug_anchor,
};

use super::backend::Backend;

// -------------------------------------
// src/lsp/diagnostics.rs
//
// fn did_open()                     L32
// fn did_change()                   L37
// fn did_close()                    L42
// fn refresh_open_documents()       L50
// fn publish_for_uri()              L56
// fn collect_link_diagnostics()     L95
// fn link_error_reason()           L117
// fn link_range()                  L158
// -------------------------------------

/// Called when a document is opened. Publish diagnostics immediately.
pub(super) async fn did_open(backend: &Backend, params: DidOpenTextDocumentParams) {
    publish_for_uri(backend, &params.text_document.uri).await;
}

/// Called when a document changes. Re-publish diagnostics.
pub(super) async fn did_change(backend: &Backend, params: DidChangeTextDocumentParams) {
    publish_for_uri(backend, &params.text_document.uri).await;
}

/// Called when a document is closed. Clear diagnostics for that document.
pub(super) async fn did_close(backend: &Backend, params: DidCloseTextDocumentParams) {
    backend
        .client
        .publish_diagnostics(params.text_document.uri, Vec::new(), None)
        .await;
}

/// Re-publish diagnostics for every currently open markdown document.
pub(super) async fn refresh_open_documents(backend: &Backend) {
    for uri in backend.open_document_uris().await {
        publish_for_uri(backend, &uri).await;
    }
}

async fn publish_for_uri(backend: &Backend, uri: &Url) {
    let Some((abs_path, rel_path)) = backend.markdown_rel_path(uri) else {
        return;
    };

    let Some(content) = backend.document_text(uri, &abs_path).await else {
        backend
            .client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
        return;
    };

    let parsed = parse_markdown(&content);
    if !backend.ensure_index().await {
        backend
            .client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
        return;
    }

    let Some(diagnostics) = backend
        .with_index(|index| collect_link_diagnostics(index, &parsed, &rel_path))
        .await
    else {
        backend
            .client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
        return;
    };

    backend
        .client
        .publish_diagnostics(uri.clone(), diagnostics, None)
        .await;
}

fn collect_link_diagnostics(
    index: &VaultIndex,
    parsed: &ParsedDocument,
    source_file: &Path,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    for link in &parsed.links {
        if let Some(reason) = link_error_reason(index, parsed, source_file, link) {
            out.push(Diagnostic {
                range: link_range(link),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("kdb".to_string()),
                message: reason,
                ..Diagnostic::default()
            });
        }
    }

    out
}

fn link_error_reason(
    index: &VaultIndex,
    parsed: &ParsedDocument,
    source_file: &Path,
    link: &Link,
) -> Option<String> {
    let Some(target_file) = resolve_target_path(source_file, link.kind, &link.target) else {
        return Some("target resolves outside root".to_string());
    };

    let target_exists = target_file == source_file || index.files.contains_key(&target_file);
    if !target_exists {
        return Some(format!("target file not found: {}", target_file.display()));
    }

    if let Some(raw_anchor) = link.target.anchor.as_deref() {
        let wanted = slug_anchor(raw_anchor);
        let heading_exists = if target_file == source_file {
            parsed
                .headings
                .iter()
                .any(|heading| heading.anchor == wanted)
        } else {
            index
                .files
                .get(&target_file)
                .is_some_and(|file| file.headings.iter().any(|heading| heading.anchor == wanted))
        };

        if !heading_exists {
            return Some(format!(
                "target heading not found: {}#{}",
                target_file.display(),
                wanted
            ));
        }
    }

    None
}

fn link_range(link: &Link) -> Range {
    let line = link.line.saturating_sub(1) as u32;
    let start_col = link.column.saturating_sub(1) as u32;
    let width = link.raw.chars().count().max(1) as u32;
    Range {
        start: Position::new(line, start_col),
        end: Position::new(line, start_col + width),
    }
}
