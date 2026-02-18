//! Go-to-definition for markdown links.
//!
//! When the cursor is on a link (markdown or wikilink), resolves the target
//! file and heading, then returns the location so the editor can jump there.

use regex::Regex;
use std::sync::LazyLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, Url,
};

use crate::index::{
    LinkKind, LinkTarget, parse_markdown_target, parse_wikilink_target, resolve_target_path,
    slug_anchor,
};

use super::backend::{Backend, position_to_byte_offset};

/// Regex matching `[text](target)` markdown links.
static MARKDOWN_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[[^\]\r\n]*\]\(([^)\r\n]+)\)").expect("valid markdown link regex")
});

/// Regex matching `[[target]]` wikilinks.
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]\r\n]+)\]\]").expect("valid wikilink regex"));

/// Handle a go-to-definition request.
///
/// Finds the link under the cursor, resolves it against the vault index,
/// and returns the target location (file + heading position).
pub(super) async fn goto_definition(
    backend: &Backend,
    params: GotoDefinitionParams,
) -> LspResult<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let Some((source_abs, source_rel)) = backend.markdown_rel_path(&uri) else {
        return Ok(None);
    };

    let Some(content) = backend.document_text(&uri, &source_abs).await else {
        return Ok(None);
    };

    let Some((kind, target)) = link_under_position(&content, position) else {
        return Ok(None);
    };

    let Some(target_rel) = resolve_target_path(&source_rel, kind, &target) else {
        return Ok(None);
    };

    if !backend.ensure_index().await {
        return Ok(None);
    };

    let Some(resolve_result) = backend
        .with_index(|index| {
            let target_file = index.files.get(&target_rel)?;

            let target_pos = if let Some(anchor) = target.anchor.as_deref() {
                let wanted = slug_anchor(anchor);
                let heading = target_file
                    .headings
                    .iter()
                    .find(|heading| heading.anchor == wanted)?;
                Position::new(
                    heading.line.saturating_sub(1) as u32,
                    heading.column.saturating_sub(1) as u32,
                )
            } else {
                Position::new(0, 0)
            };

            Some((target_file.abs_path.clone(), target_pos))
        })
        .await
    else {
        return Ok(None);
    };

    let Some((target_abs_path, target_pos)) = resolve_result else {
        return Ok(None);
    };

    let Some(target_uri) = Url::from_file_path(&target_abs_path).ok() else {
        return Ok(None);
    };

    Ok(Some(GotoDefinitionResponse::Scalar(Location {
        uri: target_uri,
        range: Range {
            start: target_pos,
            end: target_pos,
        },
    })))
}

/// Find the link (if any) that the cursor position falls within.
///
/// Scans all markdown links and wikilinks in the content and returns the
/// one whose span contains the given byte offset.
pub(super) fn link_under_position(
    content: &str,
    position: Position,
) -> Option<(LinkKind, LinkTarget)> {
    let offset = position_to_byte_offset(content, position)?;

    for captures in MARKDOWN_LINK_RE.captures_iter(content) {
        let full = captures.get(0)?;
        let target = captures.get(1)?;
        if offset >= full.start() && offset <= full.end() {
            return parse_markdown_target(target.as_str())
                .map(|target| (LinkKind::Markdown, target));
        }
    }

    for captures in WIKILINK_RE.captures_iter(content) {
        let full = captures.get(0)?;
        let target = captures.get(1)?;
        if offset >= full.start() && offset <= full.end() {
            return parse_wikilink_target(target.as_str())
                .map(|target| (LinkKind::Wikilink, target));
        }
    }

    None
}
