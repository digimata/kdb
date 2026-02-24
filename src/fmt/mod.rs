//! Code index formatter for supported source files.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::index::normalize_rel_path;
use crate::symbols::{
    CodeLanguage, Symbol, SymbolKind, extract_symbols, keyword_for_kind, language_for_path,
};

pub mod preamble;

use self::preamble::{comment_prefix, preamble_end_index};

// -------------------------------------------
// ## Index
//
// struct FormatReport                     L67
// struct FormatWarning                    L75
// struct RewriteResult                    L81
// fn format_workspace()                   L89
// fn rewrite_code_index()                L144
// fn removal_warning_message()           L241
// fn find_managed_block()                L261
// fn is_index_body_line()                L291
// fn is_canonical_index_body_line()      L303
// fn is_separator_only_comment_line()    L329
// fn render_block()                      L343
// fn build_ignore_globset()              L393
// fn discover_code_files()               L408
// fn rel_path_from_root()                L468
// fn path_is_ignored()                   L475
// -------------------------------------------

const INDEX_HEADER: &str = "## Index";
const LINE_GAP: usize = 4;
const CANONICAL_KEYWORDS: &[&str] = &[
    "fn",
    "struct",
    "enum",
    "trait",
    "type",
    "class",
    "interface",
];

/// Directories to skip during formatting discovery.
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    ".cache",
    "vendor",
    "__pycache__",
    ".venv",
    ".kdb",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormatReport {
    pub scanned_files: usize,
    pub updated_files: usize,
    pub warnings: Vec<FormatWarning>,
}

/// Non-fatal issue encountered while normalizing index blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatWarning {
    pub rel_path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RewriteResult {
    content: String,
    removed_blocks: usize,
    removed_noncanonical_rows: usize,
}

/// Walk a workspace, rewriting each supported source file with an up-to-date
/// symbol index block inserted after the preamble.
pub fn format_workspace(root: &Path, ignore_patterns: &[String]) -> Result<FormatReport> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let ignore_set = build_ignore_globset(ignore_patterns)?;
    let files = discover_code_files(&root, &ignore_set)?;

    let mut report = FormatReport {
        scanned_files: files.len(),
        updated_files: 0,
        warnings: Vec::new(),
    };

    for rel_path in files {
        let Some(language) = language_for_path(&rel_path) else {
            continue;
        };
        let abs_path = root.join(&rel_path);
        let source = match fs::read_to_string(&abs_path) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::InvalidData => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", abs_path.display()));
            }
        };

        let rewrite = rewrite_code_index(language, &source).with_context(|| {
            format!(
                "failed to rewrite code index for {}",
                rel_path.to_string_lossy()
            )
        })?;

        if let Some(message) = removal_warning_message(&rewrite) {
            report.warnings.push(FormatWarning {
                rel_path: rel_path.clone(),
                message,
            });
        }

        let formatted = rewrite.content;

        if formatted != source {
            fs::write(&abs_path, formatted)
                .with_context(|| format!("failed to write {}", abs_path.display()))?;
            report.updated_files += 1;
        }
    }

    Ok(report)
}

/// Parse `source` for symbols, strip any existing index block, and return the
/// file contents with a freshly generated index inserted after the preamble.
fn rewrite_code_index(language: CodeLanguage, source: &str) -> Result<RewriteResult> {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let had_trailing_newline = source.ends_with('\n');

    let mut lines = source
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();

    if had_trailing_newline && lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.len() == 1 && lines[0].is_empty() {
        lines.clear();
    }

    let prefix = comment_prefix(language);
    let mut removed_blocks = 0usize;
    let mut removed_noncanonical_rows = 0usize;
    loop {
        let search_limit = preamble_end_index(language, &lines)
            .max(256)
            .min(lines.len());
        let Some((start, end_exclusive)) = find_managed_block(&lines, prefix, search_limit) else {
            break;
        };

        let mut drain_start = start;
        while drain_start > 0 && is_separator_only_comment_line(&lines[drain_start - 1], prefix) {
            drain_start -= 1;
        }

        removed_blocks += 1;
        removed_noncanonical_rows += lines[start + 1..end_exclusive]
            .iter()
            .filter(|line| !is_canonical_index_body_line(line, prefix))
            .count();

        lines.drain(drain_start..end_exclusive);
    }

    let insertion_index = preamble_end_index(language, &lines);

    let mut parse_source = lines.join("\n");
    if had_trailing_newline && !parse_source.is_empty() {
        parse_source.push('\n');
    }

    let mut symbols = extract_symbols(language, &parse_source)?;
    symbols.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then_with(|| left.name.cmp(&right.name))
    });

    let insertion_line = insertion_index + 1;
    let inserted_line_count = if symbols.is_empty() {
        3
    } else {
        symbols.len() + 5
    };
    let shifted_symbols = symbols
        .into_iter()
        .map(|symbol| {
            let shifted_line = if symbol.line >= insertion_line {
                symbol.line + inserted_line_count
            } else {
                symbol.line
            };
            Symbol {
                line: shifted_line,
                ..symbol
            }
        })
        .collect::<Vec<_>>();

    let mut output_lines = Vec::new();
    output_lines.extend_from_slice(&lines[..insertion_index]);
    output_lines.extend(render_block(prefix, &shifted_symbols));
    output_lines.extend_from_slice(&lines[insertion_index..]);

    let mut output = output_lines.join(newline);
    if had_trailing_newline && !output.is_empty() {
        output.push_str(newline);
    }

    Ok(RewriteResult {
        content: output,
        removed_blocks,
        removed_noncanonical_rows,
    })
}

fn removal_warning_message(rewrite: &RewriteResult) -> Option<String> {
    if rewrite.removed_noncanonical_rows > 0 {
        return Some(format!(
            "removed {} non-standard index row(s) across {} detected index block(s)",
            rewrite.removed_noncanonical_rows, rewrite.removed_blocks
        ));
    }

    if rewrite.removed_blocks > 1 {
        return Some(format!(
            "removed {} index blocks and kept a single regenerated block",
            rewrite.removed_blocks
        ));
    }

    None
}

/// Locate a managed `## Index` comment block within the first `search_limit`
/// lines. Returns inclusive start and exclusive end indices, or `None`.
fn find_managed_block(
    lines: &[String],
    prefix: &str,
    search_limit: usize,
) -> Option<(usize, usize)> {
    let header = format!("{prefix} {INDEX_HEADER}");
    let region_end = search_limit.min(lines.len());

    if let Some(start) = lines
        .iter()
        .take(region_end)
        .position(|line| line.trim() == header)
    {
        let mut end = start + 1;
        while end < region_end && is_index_body_line(&lines[end], prefix) {
            end += 1;
        }

        if end < lines.len() && lines[end].trim().is_empty() {
            end += 1;
        }

        return Some((start, end));
    }

    None
}

/// Return `true` if `line` looks like a row inside a managed index block
/// (either a blank comment line or any prefixed comment row).
fn is_index_body_line(line: &str, prefix: &str) -> bool {
    let trimmed = line.trim();
    if trimmed == prefix {
        return true;
    }

    trimmed
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix(' '))
        .is_some()
}

fn is_canonical_index_body_line(line: &str, prefix: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed == prefix {
        return true;
    }

    let Some(rest) = trimmed
        .strip_prefix(prefix)
        .and_then(|value| value.strip_prefix(' '))
    else {
        return false;
    };

    let text = rest.trim_start();
    if text.chars().all(|ch| ch == '-') && !text.is_empty() {
        return true;
    }

    CANONICAL_KEYWORDS
        .iter()
        .any(|keyword| text.starts_with(&format!("{keyword} ")) && text.contains(" L"))
}

fn is_separator_only_comment_line(line: &str, prefix: &str) -> bool {
    let trimmed = line.trim();
    let Some(rest) = trimmed
        .strip_prefix(prefix)
        .and_then(|value| value.strip_prefix(' '))
    else {
        return false;
    };

    let text = rest.trim_start();
    !text.is_empty() && text.chars().all(|ch| ch == '-')
}

/// Render the `## Index` header and symbol rows as comment lines.
fn render_block(prefix: &str, symbols: &[Symbol]) -> Vec<String> {
    let mut rows = Vec::new();
    for symbol in symbols {
        let indent = if matches!(symbol.kind, SymbolKind::Method) {
            "  "
        } else {
            ""
        };
        let keyword = keyword_for_kind(symbol.kind);
        let qualified_name = match (&symbol.parent, symbol.kind) {
            (Some(parent), SymbolKind::Method) => format!("{parent}::{}", symbol.name),
            _ => symbol.name.clone(),
        };
        let display_name = if matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
            format!("{qualified_name}()")
        } else {
            qualified_name
        };

        rows.push((
            format!("{indent}{keyword} {display_name}"),
            format!("L{}", symbol.line),
        ));
    }

    let left_width = rows.iter().map(|(left, _)| left.len()).max().unwrap_or(0);
    let right_width = rows.iter().map(|(_, right)| right.len()).max().unwrap_or(0);
    let separator = "-".repeat(left_width + LINE_GAP + right_width);
    let gap = " ".repeat(LINE_GAP);
    let has_rows = !rows.is_empty();

    let mut lines = Vec::new();
    if has_rows {
        lines.push(format!("{prefix} {separator}"));
    }
    lines.push(format!("{prefix} {INDEX_HEADER}"));
    lines.push(prefix.to_string());
    for (left, line_label) in rows {
        lines.push(format!(
            "{prefix} {left:<left_width$}{gap}{line_label:>right_width$}"
        ));
    }
    if has_rows {
        lines.push(format!("{prefix} {separator}"));
    }
    lines.push(String::new());
    lines
}

/// Compile user-supplied ignore glob patterns into a `GlobSet`.
fn build_ignore_globset(ignore_patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in ignore_patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("invalid ignore pattern `{pattern}`"))?;
        builder.add(glob);
    }

    builder.build().context("failed to compile ignore patterns")
}

/// Recursively walk `root`, returning sorted relative paths for every
/// source file whose extension maps to a supported language.
fn discover_code_files(root: &Path, ignore_set: &GlobSet) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            let Some(rel) = rel_path_from_root(root, entry.path()) else {
                return false;
            };
            if rel.as_os_str().is_empty() {
                return true;
            }

            let name = entry.file_name().to_string_lossy();
            if IGNORED_DIRS.contains(&name.as_ref()) {
                return false;
            }

            !path_is_ignored(ignore_set, &rel, true)
        })
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let rel = entry.path().strip_prefix(root).with_context(|| {
            format!(
                "failed to strip root {} from {}",
                root.display(),
                entry.path().display()
            )
        })?;
        let rel = normalize_rel_path(rel).with_context(|| {
            format!(
                "code path {} resolves outside root {}",
                entry.path().display(),
                root.display()
            )
        })?;

        if path_is_ignored(ignore_set, &rel, false) {
            continue;
        }

        if language_for_path(&rel).is_some() {
            paths.push(rel);
        }
    }

    paths.sort();
    Ok(paths)
}

/// Strip `root` from `path` and normalize the result.
fn rel_path_from_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}

/// Check whether `rel_path` matches any pattern in the ignore set.
/// For directories, also tests the path with a trailing slash.
fn path_is_ignored(ignore_set: &GlobSet, rel_path: &Path, is_dir: bool) -> bool {
    let slash = rel_path.to_string_lossy().replace('\\', "/");
    if slash.is_empty() {
        return false;
    }

    if ignore_set.is_match(&slash) {
        return true;
    }

    if is_dir {
        return ignore_set.is_match(format!("{slash}/"));
    }

    false
}
