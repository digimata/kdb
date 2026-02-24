use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use crate::config;
use crate::index::VaultIndex;

use super::render::{self, SymbolBodyRow, SymbolRow};
use super::{Symbol, extract_symbol_body, extract_symbols, language_for_path};

// ------------------------------------
// src/symbols/query.rs
//
// pub fn collect_rows()            L23
// pub fn collect_body_rows()       L75
// struct SymbolSelector           L146
//   fn parse()                    L152
//   fn matches()                  L179
//   fn display()                  L190
// fn normalize_selector_name()    L198
// ------------------------------------

pub fn collect_rows(root: &Path, file_abs: &Path, rel_path: &Path) -> Result<Vec<SymbolRow>> {
    let is_markdown = rel_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));

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
    } else if let Some(language) = language_for_path(rel_path) {
        let source = fs::read_to_string(file_abs)
            .with_context(|| format!("failed to read {}", file_abs.display()))?;
        let mut symbols = extract_symbols(language, &source)?;
        symbols.sort_by(|left, right| {
            left.line
                .cmp(&right.line)
                .then_with(|| left.name.cmp(&right.name))
        });
        symbols.into_iter().map(render::code_symbol_row).collect()
    } else {
        bail!("unsupported file type for symbols: {}", rel_path.display());
    };

    Ok(rows)
}

pub fn collect_body_rows(
    file_abs: &Path,
    rel_path: &Path,
    selector: &str,
    public_only: bool,
) -> Result<Vec<SymbolBodyRow>> {
    if rel_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        bail!(
            "symbol body extraction is only supported for code files: {}",
            rel_path.display()
        );
    }

    let Some(language) = language_for_path(rel_path) else {
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
    let rows = symbols
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
            Ok(render::code_symbol_body_row(&file, symbol, body))
        })
        .collect::<Result<Vec<_>>>()?;

    if rows.is_empty() {
        if public_only {
            bail!(
                "symbol not found: {} in {} (after --public filter)",
                selector.display(),
                rel_path.display()
            );
        }

        bail!(
            "symbol not found: {} in {}",
            selector.display(),
            rel_path.display()
        );
    }

    Ok(rows)
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
