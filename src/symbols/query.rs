use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use crate::index::{VaultIndex, parse_markdown, section_byte_bounds, section_line_bounds};
use crate::lang::CodeLanguage;
use crate::project::config;

use super::display::{self, SymbolBodyRow, SymbolRow};
use super::{Symbol, extract_symbol_body, extract_symbols};

// ----------------------------------------
// src/symbols/query.rs
//
// pub fn collect_rows()                L29
// pub fn collect_body_rows()           L79
// fn collect_code_body_rows()         L111
// fn collect_markdown_body_rows()     L152
// fn is_markdown_file()               L198
// fn normalize_markdown_selector()    L204
// struct SymbolSelector               L219
//   fn parse()                        L225
//   fn matches()                      L252
//   fn display()                      L263
// fn normalize_selector_name()        L271
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

/// Collect full symbol bodies matching a selector for `kdb symbols -s`.
pub fn collect_body_rows(
    file_abs: &Path,
    rel_path: &Path,
    selector: &str,
    public_only: bool,
) -> Result<Vec<SymbolBodyRow>> {
    let selector_display = selector.trim();
    let rows = if is_markdown_file(rel_path) {
        collect_markdown_body_rows(file_abs, rel_path, selector)?
    } else {
        collect_code_body_rows(file_abs, rel_path, selector, public_only)?
    };

    if rows.is_empty() {
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
            let body = extract_symbol_body(&source, &symbol).with_context(|| {
                format!(
                    "failed to extract body for symbol `{}` in {}",
                    selector.display(),
                    rel_path.display()
                )
            })?;
            Ok(display::code_symbol_body_row(&file, symbol, body))
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
    fn parse(selector: &str) -> Result<Self> {
        let trimmed = selector.trim();
        if trimmed.is_empty() {
            bail!("symbol selector cannot be empty");
        }

        if let Some((parent, name)) = trimmed.rsplit_once("::") {
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
