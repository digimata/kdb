//! Document formatting for supported code files.
//!
//! This powers editor format-on-save flows by regenerating the managed code
//! index header block for the current document.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{DocumentFormattingParams, MessageType, Position, Range, TextEdit};

use crate::fmt;

use super::backend::Backend;

// ---------------------------------------
// src/lsp/formatting.rs
//
// pub(super) async fn formatting()    L21
// fn full_document_range()            L62
// ---------------------------------------

/// Handle `textDocument/formatting` for supported code and markdown files.
pub(super) async fn formatting(
    backend: &Backend,
    params: DocumentFormattingParams,
) -> LspResult<Option<Vec<TextEdit>>> {
    let uri = params.text_document.uri;
    let resolved = backend
        .code_rel_path(&uri)
        .or_else(|| backend.markdown_rel_path(&uri));
    let Some((abs_path, rel_path)) = resolved else {
        return Ok(None);
    };

    let Some(source) = backend.document_text(&uri, &abs_path).await else {
        return Ok(None);
    };

    let formatted = match fmt::format_source(&rel_path, &source) {
        Ok(Some(content)) => content,
        Ok(None) => return Ok(None),
        Err(error) => {
            backend
                .client
                .log_message(
                    MessageType::ERROR,
                    format!("failed to format {}: {error:#}", rel_path.display()),
                )
                .await;
            return Ok(None);
        }
    };

    if formatted == source {
        return Ok(Some(Vec::new()));
    }

    Ok(Some(vec![TextEdit {
        range: full_document_range(&source),
        new_text: formatted,
    }]))
}

fn full_document_range(source: &str) -> Range {
    let lines = source.split('\n').collect::<Vec<_>>();
    let end_line = lines.len().saturating_sub(1) as u32;
    let end_char = lines
        .last()
        .map(|line| line.chars().count() as u32)
        .unwrap_or(0);

    Range {
        start: Position::new(0, 0),
        end: Position::new(end_line, end_char),
    }
}
