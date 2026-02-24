//! Hover previews for markdown links.
//!
//! Hovering a link shows a short markdown preview from the target section.
//! Preview text starts at the target heading and runs until the next heading,
//! then is truncated to a fixed character limit.

use regex::{Captures, Regex};
use std::path::Path;
use std::sync::LazyLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Url};

use crate::index::{
    LinkKind, ParsedDocument, parse_markdown, parse_markdown_target, parse_wikilink_target,
    resolve_target_path, slug_anchor,
};

use super::{
    backend::{Backend, path_to_slash},
    definition::link_under_position,
};

// -----------------------------------
// src/lsp/hover.rs
//
// const HOVER_CHAR_LIMIT          L39
// static MARKDOWN_LINK_RE         L40
// static WIKILINK_RE              L43
// pub(super) async fn hover()     L50
// fn rewrite_preview_links()     L117
// fn resolve_target_url()        L171
// fn is_external_link()          L190
// fn section_preview()           L197
// fn section_line_bounds()       L217
// fn line_start_offsets()        L245
// fn truncate_chars()            L255
// -----------------------------------

const HOVER_CHAR_LIMIT: usize = 420;
static MARKDOWN_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]\r\n]*)\]\(([^)\r\n]+)\)").expect("valid markdown link regex")
});
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]\r\n]+)\]\]").expect("valid wikilink regex"));

/// Handle hover for markdown links.
///
/// If the cursor is over a link, resolves its target and returns a short
/// section preview from the destination file.
pub(super) async fn hover(backend: &Backend, params: HoverParams) -> LspResult<Option<Hover>> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let Some((source_abs, source_rel)) = backend.markdown_rel_path(&uri) else {
        return Ok(None);
    };

    let Some(source_content) = backend.document_text(&uri, &source_abs).await else {
        return Ok(None);
    };

    let Some((kind, target)) = link_under_position(&source_content, position) else {
        return Ok(None);
    };

    let Some(target_rel) = resolve_target_path(&source_rel, kind, &target) else {
        return Ok(None);
    };

    if !backend.ensure_index().await {
        return Ok(None);
    }

    let Some(target_file_abs) = backend
        .with_index(|index| {
            index
                .files
                .get(&target_rel)
                .map(|entry| entry.abs_path.clone())
        })
        .await
        .flatten()
    else {
        return Ok(None);
    };

    let Some(target_uri) = Url::from_file_path(&target_file_abs).ok() else {
        return Ok(None);
    };

    let Some(target_content) = backend.document_text(&target_uri, &target_file_abs).await else {
        return Ok(None);
    };

    let parsed = parse_markdown(&target_content);
    let Some(preview) = section_preview(&target_content, &parsed, target.anchor.as_deref()) else {
        return Ok(None);
    };

    let mut label = path_to_slash(&target_rel);
    if let Some(anchor) = target.anchor.as_deref() {
        label.push('#');
        label.push_str(&slug_anchor(anchor));
    }

    let preview = rewrite_preview_links(&preview, &backend.root, &target_rel);

    Ok(Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("`{label}`\n\n{preview}"),
        }),
        range: None,
    }))
}

fn rewrite_preview_links(snippet: &str, root: &Path, source_rel: &Path) -> String {
    let with_wikilinks = WIKILINK_RE.replace_all(snippet, |captures: &Captures<'_>| {
        let Some(full) = captures.get(0).map(|m| m.as_str()) else {
            return String::new();
        };
        let Some(inner) = captures.get(1).map(|m| m.as_str()) else {
            return full.to_string();
        };

        let target_raw = inner.split('|').next().unwrap_or(inner).trim();
        let alias = inner
            .split_once('|')
            .map(|(_, alias)| alias.trim())
            .filter(|alias| !alias.is_empty());

        let Some(target) = parse_wikilink_target(target_raw) else {
            return full.to_string();
        };
        let Some(url) = resolve_target_url(root, source_rel, LinkKind::Wikilink, &target) else {
            return full.to_string();
        };

        let label = alias.unwrap_or(target_raw);
        format!("[{label}]({url})")
    });

    MARKDOWN_LINK_RE
        .replace_all(&with_wikilinks, |captures: &Captures<'_>| {
            let Some(full) = captures.get(0).map(|m| m.as_str()) else {
                return String::new();
            };
            let Some(text) = captures.get(1).map(|m| m.as_str()) else {
                return full.to_string();
            };
            let Some(target_raw) = captures.get(2).map(|m| m.as_str()) else {
                return full.to_string();
            };

            if is_external_link(target_raw) {
                return full.to_string();
            }

            let Some(target) = parse_markdown_target(target_raw) else {
                return full.to_string();
            };
            let Some(url) = resolve_target_url(root, source_rel, LinkKind::Markdown, &target)
            else {
                return full.to_string();
            };
            format!("[{text}]({url})")
        })
        .into_owned()
}

fn resolve_target_url(
    root: &Path,
    source_rel: &Path,
    kind: LinkKind,
    target: &crate::index::LinkTarget,
) -> Option<Url> {
    let rel = resolve_target_path(source_rel, kind, target)?;
    let abs = root.join(&rel);
    if !abs.exists() {
        return None;
    }

    let mut url = Url::from_file_path(abs).ok()?;
    if let Some(anchor) = target.anchor.as_deref() {
        url.set_fragment(Some(&slug_anchor(anchor)));
    }
    Some(url)
}

fn is_external_link(raw: &str) -> bool {
    raw.contains("://")
        || raw.starts_with("mailto:")
        || raw.starts_with("tel:")
        || raw.starts_with("data:")
}

fn section_preview(content: &str, parsed: &ParsedDocument, anchor: Option<&str>) -> Option<String> {
    let (start_line, end_line) = section_line_bounds(parsed, anchor)?;
    let line_starts = line_start_offsets(content);
    let start = line_starts.get(start_line).copied().unwrap_or(0);
    let end = end_line
        .and_then(|line| line_starts.get(line).copied())
        .unwrap_or(content.len());

    if end <= start {
        return None;
    }

    let snippet = content[start..end].trim();
    if snippet.is_empty() {
        return None;
    }

    Some(truncate_chars(snippet, HOVER_CHAR_LIMIT))
}

fn section_line_bounds(
    parsed: &ParsedDocument,
    anchor: Option<&str>,
) -> Option<(usize, Option<usize>)> {
    if parsed.headings.is_empty() {
        return Some((0, None));
    }

    let start_index = match anchor {
        Some(anchor) => {
            let wanted = slug_anchor(anchor);
            parsed
                .headings
                .iter()
                .position(|heading| heading.anchor == wanted)?
        }
        None => 0,
    };

    let start_line = parsed.headings[start_index].line.saturating_sub(1);
    let end_line = parsed
        .headings
        .get(start_index + 1)
        .map(|heading| heading.line.saturating_sub(1));

    Some((start_line, end_line))
}

fn line_start_offsets(content: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in input.chars().enumerate() {
        if index == max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}
