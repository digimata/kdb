use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use crate::config;
use crate::index::VaultIndex;

use super::render::{self, SymbolRow};
use super::{extract_symbols, language_for_path};

// ------------------------
// src/symbols/query.rs
//
// fn collect_rows()    L17
// ------------------------

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
                name: heading.title.clone(),
                line: heading.line,
                parent: None,
                level: Some(heading.level),
                anchor: Some(heading.anchor.clone()),
                is_public: true,
            })
            .collect()
    } else if let Some(language) = language_for_path(&rel_path) {
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
