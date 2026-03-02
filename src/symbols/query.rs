use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::index::{VaultIndex, parse_markdown, section_byte_bounds, section_line_bounds};
use crate::lang::CodeLanguage;
use crate::project::{self, ProjectContext, config};

use super::display::{self, SymbolBodyRow, SymbolRow};
use super::{Symbol, extract_symbols};

// ----------------------------------------
// src/symbols/query.rs
//
// pub fn collect_rows()                L34
// pub fn collect_body_rows()           L84
// fn collect_code_body_rows()         L123
// fn collect_markdown_body_rows()     L167
// fn is_markdown_file()               L213
// fn normalize_markdown_selector()    L219
// struct SymbolSelector               L234
//   fn parse()                        L241
//   fn matches()                      L273
//   fn display()                      L284
// fn normalize_selector_name()        L292
// pub fn expand_paths()               L301
// fn rel_from_root()                  L343
// fn is_symbol_file()                 L368
// ----------------------------------------

/// Collect symbol rows for a single file (headings for markdown, declarations for code).
pub fn collect_rows(root: &Path, file_abs: &Path, rel_path: &Path) -> Result<Vec<SymbolRow>> {
    let is_markdown = is_markdown_file(rel_path);

    let rows: Vec<SymbolRow> = if is_markdown {
        let ignore_patterns = config::load_index_ignores(root)?;
        let index = VaultIndex::build_with_ignores(root, &ignore_patterns)?;
        let file_entry = index.files.get(rel_path).with_context(|| {
            format!(
                "file {} is not an indexed markdown file",
                rel_path.display()
            )
        })?;

        file_entry
            .headings
            .iter()
            .map(|heading| SymbolRow {
                display: format!(
                    "{} {}",
                    "#".repeat(usize::from(heading.level)),
                    heading.title
                ),
                kind: "heading".to_string(),
                display_kind: None,
                name: heading.title.clone(),
                line: heading.line,
                parent: None,
                level: Some(heading.level),
                anchor: Some(heading.anchor.clone()),
                is_public: true,
            })
            .collect()
    } else if let Some(language) = CodeLanguage::from_path(rel_path) {
        let source = fs::read_to_string(file_abs)
            .with_context(|| format!("failed to read {}", file_abs.display()))?;
        let mut symbols = extract_symbols(language, &source)?;
        symbols.sort_by(|left, right| {
            left.line
                .cmp(&right.line)
                .then_with(|| left.name.cmp(&right.name))
        });
        symbols.into_iter().map(SymbolRow::from).collect()
    } else {
        bail!("unsupported file type for symbols: {}", rel_path.display());
    };

    Ok(rows)
}

/// Collect full symbol bodies matching one or more selectors for `kdb symbols -s`.
pub fn collect_body_rows(
    file_abs: &Path,
    rel_path: &Path,
    selectors: &[&str],
    public_only: bool,
) -> Result<Vec<SymbolBodyRow>> {
    assert!(!selectors.is_empty(), "selectors must not be empty");

    let mut rows = Vec::new();
    for selector in selectors {
        let selector_display = selector.trim();
        let matched = if is_markdown_file(rel_path) {
            collect_markdown_body_rows(file_abs, rel_path, selector)?
        } else {
            collect_code_body_rows(file_abs, rel_path, selector, public_only)?
        };

        if matched.is_empty() {
            if public_only {
                bail!(
                    "symbol not found: {} in {} (after --public filter)",
                    selector_display,
                    rel_path.display()
                );
            }

            bail!(
                "symbol not found: {} in {}",
                selector_display,
                rel_path.display()
            );
        }

        rows.extend(matched);
    }

    Ok(rows)
}

fn collect_code_body_rows(
    file_abs: &Path,
    rel_path: &Path,
    selector: &str,
    public_only: bool,
) -> Result<Vec<SymbolBodyRow>> {
    let Some(language) = CodeLanguage::from_path(rel_path) else {
        bail!("unsupported file type for symbols: {}", rel_path.display());
    };

    let source = fs::read_to_string(file_abs)
        .with_context(|| format!("failed to read {}", file_abs.display()))?;
    let mut symbols = extract_symbols(language, &source)?;
    symbols.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then_with(|| left.name.cmp(&right.name))
    });

    if public_only {
        symbols.retain(|symbol| symbol.is_public);
    }

    let selector = SymbolSelector::parse(selector)?;
    let file = rel_path.to_string_lossy().replace('\\', "/");
    symbols
        .into_iter()
        .filter(|symbol| selector.matches(symbol))
        .map(|symbol| {
            let (body, start_line) = display::extract_body_with_docs(&source, &symbol)
                .with_context(|| {
                    format!(
                        "failed to extract body for symbol `{}` in {}",
                        selector.display(),
                        rel_path.display()
                    )
                })?;
            let mut row = display::code_symbol_body_row(&file, symbol, body);
            row.line = start_line;
            Ok(row)
        })
        .collect::<Result<Vec<_>>>()
}

fn collect_markdown_body_rows(
    file_abs: &Path,
    rel_path: &Path,
    selector: &str,
) -> Result<Vec<SymbolBodyRow>> {
    let source = fs::read_to_string(file_abs)
        .with_context(|| format!("failed to read {}", file_abs.display()))?;
    let parsed = parse_markdown(&source);
    let selector_anchor = normalize_markdown_selector(selector)?;

    let Some(heading) = parsed
        .headings
        .iter()
        .find(|heading| heading.anchor.eq_ignore_ascii_case(&selector_anchor))
    else {
        return Ok(Vec::new());
    };

    let (start_byte, end_byte) = section_byte_bounds(&source, &parsed, Some(&heading.anchor))
        .with_context(|| {
            format!(
                "failed to extract body for symbol `{}` in {}",
                selector,
                rel_path.display()
            )
        })?;
    let (_, end_line_start) =
        section_line_bounds(&parsed, Some(&heading.anchor)).with_context(|| {
            format!(
                "failed to resolve line bounds for symbol `{}` in {}",
                selector,
                rel_path.display()
            )
        })?;

    let end_line = end_line_start
        .unwrap_or_else(|| source.lines().count())
        .max(heading.line);
    let file = rel_path.to_string_lossy().replace('\\', "/");
    let body = source[start_byte..end_byte].to_string();

    Ok(vec![display::markdown_symbol_body_row(
        &file, heading, end_line, body,
    )])
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

fn normalize_markdown_selector(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("symbol selector cannot be empty");
    }

    let normalized = trimmed.trim_start_matches('#').trim().to_ascii_lowercase();
    if normalized.is_empty() {
        bail!("invalid symbol selector: {value}");
    }

    Ok(normalized)
}

#[derive(Debug, Clone)]
struct SymbolSelector {
    parent: Option<String>,
    name: String,
}

impl SymbolSelector {
    /// Parse a qualified selector like `Parent::name`, `Parent.name`, or bare `name`.
    fn parse(selector: &str) -> Result<Self> {
        let trimmed = selector.trim();
        if trimmed.is_empty() {
            bail!("symbol selector cannot be empty");
        }

        // Try `::` first (Rust convention), then `.` (TS/JS/Python/Go convention).
        let split = trimmed
            .rsplit_once("::")
            .or_else(|| trimmed.rsplit_once('.'));

        if let Some((parent, name)) = split {
            let parent = parent.trim();
            let name = normalize_selector_name(name);
            if parent.is_empty() || name.is_empty() {
                bail!("invalid symbol selector: {selector}");
            }

            return Ok(Self {
                parent: Some(parent.to_string()),
                name,
            });
        }

        let name = normalize_selector_name(trimmed);
        if name.is_empty() {
            bail!("invalid symbol selector: {selector}");
        }

        Ok(Self { parent: None, name })
    }

    fn matches(&self, symbol: &Symbol) -> bool {
        if symbol.name != self.name {
            return false;
        }

        match &self.parent {
            Some(parent) => symbol.parent.as_deref() == Some(parent.as_str()),
            None => true,
        }
    }

    fn display(&self) -> String {
        match &self.parent {
            Some(parent) => format!("{parent}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

fn normalize_selector_name(value: &str) -> String {
    value.trim().trim_end_matches("()").to_string()
}

/// Expand a list of paths into deduplicated `(absolute, relative)` file pairs.
///
/// Files are used directly. Directories are walked recursively, collecting
/// all files with a recognized code language or markdown extension. Results
/// are sorted by relative path and deduplicated.
pub fn expand_paths(
    project: &ProjectContext,
    paths: &[PathBuf],
) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();

    for path in paths {
        let abs = project::root::make_absolute(path)?;
        if abs.is_file() {
            let abs = abs
                .canonicalize()
                .with_context(|| format!("failed to canonicalize {}", path.display()))?;
            let rel = rel_from_root(&project.root, &abs)?;
            if seen.insert(rel.clone()) {
                result.push((abs, rel));
            }
        } else if abs.is_dir() {
            let dir_abs = abs
                .canonicalize()
                .with_context(|| format!("failed to canonicalize {}", path.display()))?;
            let discovered = project::discover::discover_files(
                &project.root,
                &dir_abs,
                &project.ignore_set,
            )?;
            for rel in discovered {
                if is_symbol_file(&rel) && seen.insert(rel.clone()) {
                    let file_abs = project.root.join(&rel);
                    result.push((file_abs, rel));
                }
            }
        } else {
            bail!("path not found: {}", abs.display());
        }
    }

    result.sort_by(|(_, a), (_, b)| a.cmp(b));
    Ok(result)
}

/// Compute a root-relative path from an absolute path.
fn rel_from_root(root: &Path, abs: &Path) -> Result<PathBuf> {
    let canonical = abs
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", abs.display()))?;
    canonical
        .strip_prefix(root)
        .with_context(|| {
            format!(
                "path {} is not inside kdb root {}",
                canonical.display(),
                root.display()
            )
        })
        .and_then(|rel| {
            project::paths::normalize_rel_path(rel).with_context(|| {
                format!(
                    "path {} resolves outside kdb root {}",
                    canonical.display(),
                    root.display()
                )
            })
        })
}

/// Check whether a file is supported by the symbols command.
fn is_symbol_file(rel: &Path) -> bool {
    is_markdown_file(rel) || CodeLanguage::from_path(rel).is_some()
}
