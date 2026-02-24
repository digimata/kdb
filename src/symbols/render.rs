//! Symbol display formatting for CLI output and codemap generation.

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use super::{Symbol, SymbolKind, keyword_for_kind};

// ---------------------------
// src/symbols/render.rs
//
// struct SymbolRow        L20
// fn code_symbol_row()    L40
// fn print_text()         L72
// fn print_json()         L85
// fn row_to_json()        L93
// ---------------------------

/// A formatted symbol row ready for display.
#[derive(Debug, Clone)]
pub struct SymbolRow {
    /// Formatted display string (e.g. `"  fn Backend::new()"`).
    pub display: String,
    /// Kind keyword (e.g. `"fn"`, `"struct"`).
    pub kind: String,
    /// Raw symbol name.
    pub name: String,
    /// 1-based line number.
    pub line: usize,
    /// Parent type name for methods.
    pub parent: Option<String>,
    /// Heading level for markdown symbols.
    pub level: Option<u8>,
    /// Anchor slug for markdown headings.
    pub anchor: Option<String>,
    /// Whether the symbol is publicly visible.
    pub is_public: bool,
}

/// Convert a code symbol into a display row.
pub fn code_symbol_row(symbol: Symbol) -> SymbolRow {
    let keyword = keyword_for_kind(symbol.kind);
    let indent = if matches!(symbol.kind, SymbolKind::Method) {
        "  "
    } else {
        ""
    };

    let qualified_name = match (&symbol.parent, symbol.kind) {
        (Some(parent), SymbolKind::Method) => format!("{parent}::{}", symbol.name),
        _ => symbol.name.clone(),
    };

    let display_name = if matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
        format!("{qualified_name}()")
    } else {
        qualified_name
    };

    SymbolRow {
        display: format!("{indent}{keyword} {display_name}"),
        kind: keyword.to_string(),
        name: symbol.name,
        line: symbol.line,
        parent: symbol.parent,
        level: None,
        anchor: None,
        is_public: symbol.is_public,
    }
}

/// Print symbol rows as aligned plain text.
pub fn print_text(rows: &[SymbolRow]) {
    if rows.is_empty() {
        println!("(no symbols)");
        return;
    }

    let width = rows.iter().map(|row| row.display.len()).max().unwrap_or(0);
    for row in rows {
        println!("{:<width$}  L{}", row.display, row.line, width = width);
    }
}

/// Print symbol rows as formatted JSON.
pub fn print_json(rows: &[SymbolRow]) -> Result<()> {
    let payload = rows.iter().map(row_to_json).collect::<Vec<_>>();
    let output =
        serde_json::to_string_pretty(&payload).context("failed to serialize symbols as JSON")?;
    println!("{output}");
    Ok(())
}

fn row_to_json(row: &SymbolRow) -> Value {
    let mut object = Map::new();
    object.insert("kind".to_string(), Value::String(row.kind.clone()));
    object.insert("name".to_string(), Value::String(row.name.clone()));
    object.insert("line".to_string(), Value::from(row.line as u64));
    object.insert("public".to_string(), Value::Bool(row.is_public));

    if let Some(parent) = &row.parent {
        object.insert("parent".to_string(), Value::String(parent.clone()));
    }
    if let Some(level) = row.level {
        object.insert("level".to_string(), Value::from(level as u64));
    }
    if let Some(anchor) = &row.anchor {
        object.insert("anchor".to_string(), Value::String(anchor.clone()));
    }

    Value::Object(object)
}
