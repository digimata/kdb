//! Hover previews for markdown links.
//!
//! Hovering a link shows a short markdown preview from the target section.
//! Preview text starts at the target heading and runs until the next heading
//! of equal or higher level, then is truncated to a fixed character limit.

use regex::{Captures, Regex};
use std::path::Path;
use std::sync::LazyLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range, Url,
};

use crate::index::{
    LinkKind, ParsedDocument, parse_markdown, parse_markdown_target, parse_wikilink_target,
    resolve_target_path, section_byte_bounds, slug_anchor,
};

use super::{
    backend::{Backend, path_to_slash},
    definition::{INDEX_LINE_RE, is_in_frontmatter, link_under_position},
};

// -----------------------------------
// kdb/src/lsp/hover.rs
//
// const HOVER_CHAR_LIMIT          L40
// static MARKDOWN_LINK_RE         L41
// static WIKILINK_RE              L44
// pub(super) async fn hover()     L51
// fn rewrite_preview_links()     L130
// fn resolve_target_url()        L184
// fn outline_row_hover()         L205
// fn is_external_link()          L242
// fn section_preview()           L249
// fn truncate_chars()            L259
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

    // Try outline-row hover: show section preview for frontmatter/index rows.
    let any_abs = backend
        .markdown_rel_path(&uri)
        .or_else(|| backend.code_rel_path(&uri));
    if let Some((abs_path, _)) = &any_abs {
        if let Some(content) = backend.document_text(&uri, abs_path).await {
            if let Some(hover) = outline_row_hover(&uri, &content, position) {
                return Ok(Some(hover));
            }
        }
    }

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

/// If the cursor is on an outline/index row with a line label, show a section
/// preview from the heading at that line.
fn outline_row_hover(uri: &Url, content: &str, position: Position) -> Option<Hover> {
    let line_text = content.split('\n').nth(position.line as usize)?;
    let trimmed = line_text.trim();

    let is_index_row = trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with('>')
        || is_in_frontmatter(content, position.line as usize);
    if !is_index_row {
        return None;
    }

    let captures = INDEX_LINE_RE.captures(line_text)?;
    let target_line: usize = captures.get(1)?.as_str().parse().ok()?;

    // Find the heading at target_line and show a preview of its section.
    let parsed = parse_markdown(content);
    let heading = parsed
        .headings
        .iter()
        .find(|h| h.line == target_line)?;

    let anchor = slug_anchor(&heading.title);
    let preview = section_preview(content, &parsed, Some(&anchor))?;

    let label = format!("L{target_line}");
    let _ = uri;

    let line_len = line_text.len() as u32;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("`{label}`\n\n{preview}"),
        }),
        range: Some(Range {
            start: Position::new(position.line, 0),
            end: Position::new(position.line, line_len),
        }),
    })
}

fn is_external_link(raw: &str) -> bool {
    raw.contains("://")
        || raw.starts_with("mailto:")
        || raw.starts_with("tel:")
        || raw.starts_with("data:")
}

fn section_preview(content: &str, parsed: &ParsedDocument, anchor: Option<&str>) -> Option<String> {
    let (start, end) = section_byte_bounds(content, parsed, anchor)?;

    let snippet = content[start..end].trim();
    if snippet.is_empty() {
        return None;
    }

    Some(truncate_chars(snippet, HOVER_CHAR_LIMIT))
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
