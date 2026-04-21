//! Include syntax parsing for transclusion directives.
//!
//! Recognizes Obsidian-style embed syntax: `![[file]]`, `![[file#heading]]`,
//! `![[kdb://file#heading]]`. Resolved at render time by `kdb render`.

use regex::Regex;
use std::sync::LazyLock;

// -------------------------------------------
// projects/kdb/src/render/include.rs
//
// static EMBED_RE                         L30
// pub struct IncludeDirective             L35
// pub struct Embed                        L46
// pub fn parse_embed_target()             L68
// pub fn find_embeds()                   L129
// mod tests                              L158
// fn parse_simple_file()                 L162
// fn parse_file_with_anchor()            L170
// fn parse_kdb_scheme()                  L178
// fn parse_with_alias()                  L186
// fn parse_empty_returns_none()          L194
// fn parse_anchor_only_returns_none()    L200
// fn find_embeds_standalone_only()       L205
// fn find_embeds_empty_doc()             L221
// fn find_embeds_skips_code_fences()     L227
// -------------------------------------------

/// Matches `![[target]]` on a line by itself (optional surrounding whitespace).
static EMBED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^!\[\[([^\]]+)\]\]$").expect("valid embed regex"));

/// A parsed transclusion directive with file and optional heading anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDirective {
    /// File path (e.g. `SOP.md`, `kdb://SOP.md`).
    pub file: String,
    /// Optional heading anchor (e.g. `daily-shutdown`).
    pub anchor: Option<String>,
    /// Whether the path uses the `kdb://` root-relative scheme.
    pub root_relative: bool,
}

/// A located transclusion embed in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embed {
    /// The parsed include target.
    pub directive: IncludeDirective,
    /// 0-based line index.
    pub line: usize,
}

/// Parse the inner content of `![[...]]` into an [`IncludeDirective`].
///
/// Accepts `file.md`, `file.md#heading`, `kdb://file.md`, `kdb://file.md#heading`.
/// Supports `|alias` suffix (alias is stripped, like wikilinks).
///
/// # Examples
///
/// ```
/// use kdb::render::include::parse_embed_target;
///
/// let d = parse_embed_target("SOP.md#daily-shutdown").unwrap();
/// assert_eq!(d.file, "SOP.md");
/// assert_eq!(d.anchor.as_deref(), Some("daily-shutdown"));
/// assert!(!d.root_relative);
/// ```
pub fn parse_embed_target(raw: &str) -> Option<IncludeDirective> {
    // Strip alias (everything after `|`).
    let body = raw.split('|').next()?.trim();
    if body.is_empty() {
        return None;
    }

    let (body, root_relative) = match body.strip_prefix("kdb://") {
        Some(rest) => (rest.trim(), true),
        None => (body, false),
    };

    if body.is_empty() {
        return None;
    }

    let (file, anchor) = match body.split_once('#') {
        Some((f, a)) => {
            let f = f.trim();
            let a = a.trim();
            if f.is_empty() {
                return None;
            }
            let anchor = if a.is_empty() {
                None
            } else {
                Some(a.to_string())
            };
            (f.to_string(), anchor)
        }
        None => (body.to_string(), None),
    };

    Some(IncludeDirective {
        file,
        anchor,
        root_relative,
    })
}

/// Scan lines for `![[target]]` embeds.
///
/// Only matches lines where the embed is the sole content (trimmed).
/// Skips embeds inside fenced code blocks (` ``` ` or `~~~`).
///
/// # Examples
///
/// ```
/// use kdb::render::include::find_embeds;
///
/// let lines: Vec<&str> = vec![
///     "# Title",
///     "![[SOP.md#setup]]",
///     "some text with ![[inline.md]] link",
///     "![[other.md]]",
/// ];
/// let embeds = find_embeds(&lines);
/// assert_eq!(embeds.len(), 2);
/// assert_eq!(embeds[0].line, 1);
/// assert_eq!(embeds[1].line, 3);
/// ```
pub fn find_embeds(lines: &[&str]) -> Vec<Embed> {
    let mut results = Vec::new();
    let mut in_code_fence = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Toggle code fence state on ``` or ~~~ lines.
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            continue;
        }

        if in_code_fence {
            continue;
        }

        if let Some(caps) = EMBED_RE.captures(trimmed) {
            let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(directive) = parse_embed_target(inner) {
                results.push(Embed { directive, line: i });
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_file() {
        let d = parse_embed_target("SOP.md").unwrap();
        assert_eq!(d.file, "SOP.md");
        assert!(d.anchor.is_none());
        assert!(!d.root_relative);
    }

    #[test]
    fn parse_file_with_anchor() {
        let d = parse_embed_target("SOP.md#daily-shutdown").unwrap();
        assert_eq!(d.file, "SOP.md");
        assert_eq!(d.anchor.as_deref(), Some("daily-shutdown"));
        assert!(!d.root_relative);
    }

    #[test]
    fn parse_kdb_scheme() {
        let d = parse_embed_target("kdb://SOP.md#setup").unwrap();
        assert_eq!(d.file, "SOP.md");
        assert_eq!(d.anchor.as_deref(), Some("setup"));
        assert!(d.root_relative);
    }

    #[test]
    fn parse_with_alias() {
        let d = parse_embed_target("SOP.md#setup|Setup Procedure").unwrap();
        assert_eq!(d.file, "SOP.md");
        assert_eq!(d.anchor.as_deref(), Some("setup"));
        assert!(!d.root_relative);
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_embed_target("").is_none());
        assert!(parse_embed_target("   ").is_none());
    }

    #[test]
    fn parse_anchor_only_returns_none() {
        assert!(parse_embed_target("#heading").is_none());
    }

    #[test]
    fn find_embeds_standalone_only() {
        let lines = vec![
            "# Title",
            "![[a.md]]",
            "text with ![[b.md]] inline",
            "![[c.md#section]]",
        ];
        let embeds = find_embeds(&lines);
        assert_eq!(embeds.len(), 2);
        assert_eq!(embeds[0].directive.file, "a.md");
        assert_eq!(embeds[0].line, 1);
        assert_eq!(embeds[1].directive.file, "c.md");
        assert_eq!(embeds[1].line, 3);
    }

    #[test]
    fn find_embeds_empty_doc() {
        let lines: Vec<&str> = vec![];
        assert!(find_embeds(&lines).is_empty());
    }

    #[test]
    fn find_embeds_skips_code_fences() {
        let lines = vec![
            "![[real.md]]",
            "```markdown",
            "![[example.md]]",
            "```",
            "![[also-real.md]]",
        ];
        let embeds = find_embeds(&lines);
        assert_eq!(embeds.len(), 2);
        assert_eq!(embeds[0].directive.file, "real.md");
        assert_eq!(embeds[1].directive.file, "also-real.md");
    }
}
