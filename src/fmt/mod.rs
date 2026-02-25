//! Code index formatter for supported source files.

use anyhow::{Context, Result, bail};
use globset::GlobSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::project::discover::discover_files;
use crate::project::ignore::{ALWAYS_IGNORED_DIRS, build_ignore_globset};
use crate::lang::CodeLanguage;
use crate::symbols::{Symbol, extract_symbols, format_symbol_display};

pub mod preamble;

use self::preamble::{comment_prefix, preamble_end_index};

// -------------------------------------------
// src/fmt/mod.rs
//
// pub mod preamble                        L14
// const LEGACY_INDEX_HEADER               L43
// const LINE_GAP                          L44
// pub struct FormatReport                 L48
// pub struct FormatWarning                L56
// struct RewriteResult                    L62
// pub fn format_workspace()               L70
// pub fn format_path()                    L84
// pub fn format_source()                 L107
// fn format_files()                      L116
// fn rewrite_code_index()                L165
// fn removal_warning_message()           L275
// fn find_managed_block()                L295
// fn is_header_candidate()               L326
// fn looks_like_path_header()            L339
// fn is_index_body_line()                L359
// fn is_canonical_index_body_line()      L371
// fn is_separator_only_comment_line()    L399
// fn render_block()                      L413
// fn discover_code_files_in_scope()      L445
// -------------------------------------------

const LEGACY_INDEX_HEADER: &str = "## Index";
const LINE_GAP: usize = 4;


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
    let files = discover_code_files_in_scope(&root, &root, &ignore_set)?;

    format_files(&root, files)
}

/// Rewrite code index headers for either a single file or a directory scope.
///
/// `target` must be inside `root`. If `target` is a file, only that file is
/// considered; if it is a directory, the subtree is scanned.
pub fn format_path(root: &Path, target: &Path, ignore_patterns: &[String]) -> Result<FormatReport> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let target = target
        .canonicalize()
        .with_context(|| format!("failed to canonicalize target {}", target.display()))?;

    if !target.starts_with(&root) {
        bail!(
            "format target {} is not inside kdb root {}",
            target.display(),
            root.display()
        );
    }

    let ignore_set = build_ignore_globset(ignore_patterns)?;
    let files = discover_code_files_in_scope(&root, &target, &ignore_set)?;

    format_files(&root, files)
}

/// Rewrite a single source string for a supported code file path.
pub fn format_source(rel_path: &Path, source: &str) -> Result<Option<String>> {
    let Some(language) = CodeLanguage::from_path(rel_path) else {
        return Ok(None);
    };

    let rewrite = rewrite_code_index(language, source, rel_path)?;
    Ok(Some(rewrite.content))
}

fn format_files(root: &Path, files: Vec<PathBuf>) -> Result<FormatReport> {
    let mut report = FormatReport {
        scanned_files: files.len(),
        updated_files: 0,
        warnings: Vec::new(),
    };

    for rel_path in files {
        let Some(language) = CodeLanguage::from_path(&rel_path) else {
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

        let rewrite = rewrite_code_index(language, &source, &rel_path).with_context(|| {
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
fn rewrite_code_index(
    language: CodeLanguage,
    source: &str,
    rel_path: &Path,
) -> Result<RewriteResult> {
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
    let header = rel_path.to_string_lossy().replace('\\', "/");
    let mut removed_blocks = 0usize;
    let mut removed_noncanonical_rows = 0usize;
    loop {
        let search_limit = preamble_end_index(language, &lines)
            .max(256)
            .min(lines.len());
        let Some((start, end_exclusive)) =
            find_managed_block(&lines, prefix, &header, search_limit)
        else {
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
            let shifted_end_line = if symbol.end_line >= insertion_line {
                symbol.end_line + inserted_line_count
            } else {
                symbol.end_line
            };
            Symbol {
                line: shifted_line,
                end_line: shifted_end_line,
                ..symbol
            }
        })
        .collect::<Vec<_>>();

    let mut output_lines = Vec::new();
    output_lines.extend_from_slice(&lines[..insertion_index]);
    output_lines.extend(render_block(prefix, &header, &shifted_symbols));
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

/// Locate a managed index comment block within the first `search_limit` lines.
/// Returns inclusive start and exclusive end indices, or `None`.
fn find_managed_block(
    lines: &[String],
    prefix: &str,
    expected_header: &str,
    search_limit: usize,
) -> Option<(usize, usize)> {
    let region_end = search_limit.min(lines.len());

    for start in 0..region_end {
        if !is_header_candidate(&lines[start], prefix, expected_header) {
            continue;
        }
        if start + 1 >= region_end || !is_index_body_line(&lines[start + 1], prefix) {
            continue;
        }

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

fn is_header_candidate(line: &str, prefix: &str, expected_header: &str) -> bool {
    let trimmed = line.trim();
    let Some(rest) = trimmed
        .strip_prefix(prefix)
        .and_then(|value| value.strip_prefix(' '))
    else {
        return false;
    };

    let value = rest.trim();
    value == expected_header || value == LEGACY_INDEX_HEADER || looks_like_path_header(value)
}

fn looks_like_path_header(value: &str) -> bool {
    if value.is_empty() || value.contains(' ') {
        return false;
    }

    let normalized = value.replace('\\', "/");
    if normalized.starts_with('/') {
        return false;
    }

    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let Some((_, ext)) = file_name.rsplit_once('.') else {
        return false;
    };

    !ext.is_empty() && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
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

    let text = rest.trim();
    if text.chars().all(|ch| ch == '-') && !text.is_empty() {
        return true;
    }

    let Some((_, line_label)) = text.rsplit_once(" L") else {
        return false;
    };

    !line_label.is_empty() && line_label.chars().all(|ch| ch.is_ascii_digit())
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

/// Render the path header and symbol rows as comment lines.
fn render_block(prefix: &str, header: &str, symbols: &[Symbol]) -> Vec<String> {
    let mut rows = Vec::new();
    for symbol in symbols {
        rows.push((format_symbol_display(symbol), format!("L{}", symbol.line)));
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
    lines.push(format!("{prefix} {header}"));
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

/// Recursively walk `root`, returning sorted relative paths for every
/// source file whose extension maps to a supported language.
fn discover_code_files_in_scope(
    root: &Path,
    scope: &Path,
    ignore_set: &GlobSet,
) -> Result<Vec<PathBuf>> {
    let paths = discover_files(root, scope, ignore_set, ALWAYS_IGNORED_DIRS)?;
    Ok(paths
        .into_iter()
        .filter(|rel| CodeLanguage::from_path(rel).is_some())
        .collect())
}
