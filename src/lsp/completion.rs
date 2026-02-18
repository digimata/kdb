//! Autocomplete for markdown links.
//!
//! Provides file and heading completions when the cursor is inside a link:
//!
//! - `[text](|` or `[[|` — suggest file names.
//! - `[text](file.md#|` or `[[file#|` — suggest headings in the target file.
//!
//! Completions use relative paths so they're valid regardless of the source
//! file's location in the vault.

use std::path::Path;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Position,
};

use crate::index::{LinkKind, LinkTarget, VaultIndex, resolve_target_path, slug_anchor};

use super::backend::{
    Backend, is_markdown_path, path_to_slash, position_to_byte_offset, relative_path,
};

/// Handle a completion request by detecting the link context at the cursor
/// and returning matching file or heading suggestions.
pub(super) async fn completion(
    backend: &Backend,
    params: CompletionParams,
) -> LspResult<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let Some((source_abs, source_rel)) = backend.markdown_rel_path(&uri) else {
        return Ok(None);
    };

    let Some(content) = backend.document_text(&uri, &source_abs).await else {
        return Ok(None);
    };

    let Some(context) = completion_context(&content, position) else {
        return Ok(None);
    };

    let index = match VaultIndex::build(&backend.root) {
        Ok(index) => index,
        Err(_) => return Ok(None),
    };

    let items = match context {
        CompletionContext::File { kind, prefix } => {
            complete_files(&index, &source_rel, kind, &prefix)
        }
        CompletionContext::Heading {
            kind,
            file,
            anchor_prefix,
        } => complete_headings(&index, &source_rel, kind, file.as_deref(), &anchor_prefix),
    };

    Ok(Some(CompletionResponse::Array(items)))
}

/// What kind of completion the cursor position calls for.
enum CompletionContext {
    /// Cursor is in the file portion of a link — suggest file names.
    File { kind: LinkKind, prefix: String },
    /// Cursor is after `#` — suggest heading anchors in the target file.
    Heading {
        kind: LinkKind,
        file: Option<String>,
        anchor_prefix: String,
    },
}

/// Analyze the text before the cursor to determine the completion context.
///
/// Looks for the nearest `[[` or `](` before the cursor and parses the
/// fragment to decide whether we're completing a file name or heading anchor.
fn completion_context(content: &str, position: Position) -> Option<CompletionContext> {
    let offset = position_to_byte_offset(content, position)?;
    let line_start = content[..offset].rfind('\n').map_or(0, |index| index + 1);
    let before_cursor = &content[line_start..offset];

    let wiki_start = before_cursor.rfind("[[");
    let markdown_start = before_cursor.rfind("](");

    match (wiki_start, markdown_start) {
        (Some(wiki), Some(markdown)) if wiki > markdown => {
            parse_wikilink_completion(&before_cursor[wiki + 2..])
        }
        (Some(_), Some(markdown)) => parse_markdown_completion(&before_cursor[markdown + 2..]),
        (Some(wiki), None) => parse_wikilink_completion(&before_cursor[wiki + 2..]),
        (None, Some(markdown)) => parse_markdown_completion(&before_cursor[markdown + 2..]),
        _ => None,
    }
}

/// Parse the fragment after `](` for markdown-style link completion.
fn parse_markdown_completion(fragment: &str) -> Option<CompletionContext> {
    if fragment.contains(')') {
        return None;
    }

    let fragment = fragment.trim();
    if let Some((file, anchor)) = fragment.split_once('#') {
        return Some(CompletionContext::Heading {
            kind: LinkKind::Markdown,
            file: optional_string(file),
            anchor_prefix: anchor.to_string(),
        });
    }

    Some(CompletionContext::File {
        kind: LinkKind::Markdown,
        prefix: fragment.to_string(),
    })
}

/// Parse the fragment after `[[` for wikilink-style completion.
fn parse_wikilink_completion(fragment: &str) -> Option<CompletionContext> {
    if fragment.contains("]]") {
        return None;
    }

    let fragment = fragment.split('|').next().unwrap_or(fragment).trim();
    if let Some((file, anchor)) = fragment.split_once('#') {
        return Some(CompletionContext::Heading {
            kind: LinkKind::Wikilink,
            file: optional_string(file),
            anchor_prefix: anchor.to_string(),
        });
    }

    Some(CompletionContext::File {
        kind: LinkKind::Wikilink,
        prefix: fragment.to_string(),
    })
}

/// Convert an empty-or-whitespace string to `None`.
fn optional_string(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Generate file name completions filtered by the typed prefix.
fn complete_files(
    index: &VaultIndex,
    source_file: &Path,
    kind: LinkKind,
    prefix: &str,
) -> Vec<CompletionItem> {
    let source_dir = source_file.parent().unwrap_or(Path::new(""));
    let mut items = Vec::new();

    for rel_path in index.files.keys() {
        let mut candidate = relative_path(source_dir, rel_path);
        if matches!(kind, LinkKind::Wikilink) && is_markdown_path(&candidate) {
            candidate.set_extension("");
        }

        let label = path_to_slash(&candidate);
        if !prefix.is_empty() && !label.starts_with(prefix) {
            continue;
        }

        items.push(CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::FILE),
            detail: Some(path_to_slash(rel_path)),
            insert_text: Some(label),
            ..CompletionItem::default()
        });
    }

    items.sort_by(|left, right| left.label.cmp(&right.label));
    items
}

/// Generate heading anchor completions for a target file, filtered by prefix.
fn complete_headings(
    index: &VaultIndex,
    source_file: &Path,
    kind: LinkKind,
    file: Option<&str>,
    anchor_prefix: &str,
) -> Vec<CompletionItem> {
    let target_file = match file {
        Some(file) => {
            let target = LinkTarget {
                file: Some(file.to_string()),
                anchor: None,
            };
            if let Some(resolved) = resolve_target_path(source_file, kind, &target) {
                resolved
            } else if matches!(kind, LinkKind::Markdown) && Path::new(file).extension().is_none() {
                let target_with_md = LinkTarget {
                    file: Some(format!("{file}.md")),
                    anchor: None,
                };
                let Some(resolved) = resolve_target_path(source_file, kind, &target_with_md) else {
                    return Vec::new();
                };
                resolved
            } else {
                return Vec::new();
            }
        }
        None => source_file.to_path_buf(),
    };

    let Some(file_entry) = index.files.get(&target_file) else {
        return Vec::new();
    };

    let anchor_prefix = if anchor_prefix.trim().is_empty() {
        String::new()
    } else {
        slug_anchor(anchor_prefix)
    };

    let mut items = Vec::new();
    for heading in &file_entry.headings {
        if !anchor_prefix.is_empty() && !heading.anchor.starts_with(&anchor_prefix) {
            continue;
        }

        items.push(CompletionItem {
            label: heading.title.clone(),
            kind: Some(CompletionItemKind::TEXT),
            insert_text: Some(heading.anchor.clone()),
            ..CompletionItem::default()
        });
    }

    items.sort_by(|left, right| left.label.cmp(&right.label));
    items
}
