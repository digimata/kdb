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

// -----------------------------------------------------
// projects/kdb/src/lsp/definition.rs
//
// pub(super) static INDEX_LINE_RE                   L34
// static MARKDOWN_LINK_RE                           L46
// static WIKILINK_RE                                L51
// pub(super) async fn goto_definition()             L58
// pub(super) fn link_under_position()              L143
// fn index_line_jump()                             L174
// pub(super) fn is_in_frontmatter()                L207
// mod tests                                        L229
// fn index_line_jump_markdown_frontmatter_row()    L233
// fn index_line_jump_code_index_row()              L255
// fn index_line_jump_python_index_row()            L276
// fn index_line_jump_ignores_non_index_lines()     L288
// -----------------------------------------------------

/// Regex matching an index/nav-block outline row ending in a line label.
///
/// Matches both markdown nav rows (`> ## Heading    L42`) and code index
/// rows (`// pub fn name()    L42`, `#   def hi()    L3`).
pub(super) static INDEX_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s+L(\d+)\s*$").expect("valid index line regex")
});

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

    // Try index-row jump for any file type (markdown nav blocks + code index blocks).
    let any_abs = backend
        .markdown_rel_path(&uri)
        .or_else(|| backend.code_rel_path(&uri));
    if let Some((abs_path, _)) = &any_abs {
        if let Some(content) = backend.document_text(&uri, abs_path).await {
            if let Some(response) = index_line_jump(&uri, &content, position) {
                return Ok(Some(response));
            }
        }
    }

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

/// If the cursor is on an index/nav-block row with a line label, jump to that line.
///
/// Works for both markdown nav rows (`> ## Heading    L42`) and code index
/// rows (`// pub fn name()    L42`).
fn index_line_jump(
    uri: &Url,
    content: &str,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let line_text = content.split('\n').nth(position.line as usize)?;
    let trimmed = line_text.trim();

    // Only match lines that look like index block rows: comment-prefixed,
    // blockquote, or inside YAML frontmatter.
    let is_index_row = trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with('>')
        || is_in_frontmatter(content, position.line as usize);
    if !is_index_row {
        return None;
    }

    let captures = INDEX_LINE_RE.captures(line_text)?;
    let line_number: u32 = captures.get(1)?.as_str().parse().ok()?;
    let target_line = line_number.saturating_sub(1);

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: Range {
            start: Position::new(target_line, 0),
            end: Position::new(target_line, 0),
        },
    }))
}

/// Return `true` if `line_index` (0-based) falls inside a YAML frontmatter
/// block (`---` delimited at the top of the file).
pub(super) fn is_in_frontmatter(content: &str, line_index: usize) -> bool {
    let mut lines = content.split('\n');
    let Some(first) = lines.next() else {
        return false;
    };
    if first.trim() != "---" {
        return false;
    }
    if line_index == 0 {
        return true;
    }
    for (idx, line) in lines.enumerate() {
        let current = idx + 1; // 0-based line number
        if line.trim() == "---" || line.trim() == "..." {
            // Found closing delimiter — line_index is inside if it's before this.
            return line_index < current;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_line_jump_markdown_frontmatter_row() {
        let uri = Url::parse("file:///test.md").unwrap();
        // Frontmatter with outline containing line labels.
        let content = "---\npath: test.md\noutline: |\n  • Intro    L8\n    ◦ Details    L15\n---\n\n# Intro\n";
        // Cursor on line 3 (the `  • Intro    L8` row inside frontmatter).
        let response = index_line_jump(&uri, content, Position::new(3, 5));
        assert!(response.is_some());
        let GotoDefinitionResponse::Scalar(location) = response.unwrap() else {
            panic!("expected scalar response");
        };
        assert_eq!(location.range.start.line, 7); // L8 → 0-indexed line 7

        // Cursor on line 4 (the `    ◦ Details    L15` row).
        let response = index_line_jump(&uri, content, Position::new(4, 5));
        assert!(response.is_some());
        let GotoDefinitionResponse::Scalar(location) = response.unwrap() else {
            panic!("expected scalar response");
        };
        assert_eq!(location.range.start.line, 14); // L15 → 0-indexed line 14
    }

    #[test]
    fn index_line_jump_code_index_row() {
        let uri = Url::parse("file:///lib.rs").unwrap();
        let content = "// --------\n// lib.rs\n//\n// pub fn run()    L8\n//   fn helper()    L15\n// --------\n\npub fn run() {}\n";
        // Cursor on `// pub fn run()    L8` row.
        let response = index_line_jump(&uri, content, Position::new(3, 5));
        assert!(response.is_some());
        let GotoDefinitionResponse::Scalar(location) = response.unwrap() else {
            panic!("expected scalar response");
        };
        assert_eq!(location.range.start.line, 7);

        // Cursor on `//   fn helper()    L15` row.
        let response = index_line_jump(&uri, content, Position::new(4, 5));
        assert!(response.is_some());
        let GotoDefinitionResponse::Scalar(location) = response.unwrap() else {
            panic!("expected scalar response");
        };
        assert_eq!(location.range.start.line, 14);
    }

    #[test]
    fn index_line_jump_python_index_row() {
        let uri = Url::parse("file:///tool.py").unwrap();
        let content = "# --------\n# tool.py\n#\n# class Greeter    L8\n#   def hi()    L15\n# --------\n";
        let response = index_line_jump(&uri, content, Position::new(3, 5));
        assert!(response.is_some());
        let GotoDefinitionResponse::Scalar(location) = response.unwrap() else {
            panic!("expected scalar response");
        };
        assert_eq!(location.range.start.line, 7);
    }

    #[test]
    fn index_line_jump_ignores_non_index_lines() {
        let uri = Url::parse("file:///test.md").unwrap();
        let content = "> -----------\n> test.md\n>\n> # Intro    L8\n\n# Intro\n";

        // Blank blockquote line.
        assert!(index_line_jump(&uri, content, Position::new(2, 0)).is_none());
        // Regular content line.
        assert!(index_line_jump(&uri, content, Position::new(5, 0)).is_none());
        // Blank line.
        assert!(index_line_jump(&uri, content, Position::new(4, 0)).is_none());
    }
}
