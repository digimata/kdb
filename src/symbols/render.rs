//! Symbol display formatting for CLI output and codemap generation.

use serde::Serialize;

use crate::index::Heading;

use super::{Symbol, format_symbol_display, kind_label};

// -----------------------------------------
// src/symbols/render.rs
//
// pub struct SymbolRow                  L23
// pub struct SymbolBodyRow              L52
//   fn from()                           L76
// pub fn code_symbol_body_row()         L94
// pub fn markdown_symbol_body_row()    L109
// pub fn print_text()                  L129
// pub fn print_bodies_text()           L142
// -----------------------------------------

/// A formatted symbol row ready for display.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolRow {
    /// Formatted display string (e.g. `"  fn Backend::new()"`).
    #[serde(skip_serializing)]
    pub display: String,
    /// Stable kind label used for machine filtering.
    pub kind: String,
    /// Language-native declaration keyword chain, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_kind: Option<String>,
    /// Raw symbol name.
    pub name: String,
    /// 1-based line number.
    pub line: usize,
    /// Parent type name for methods.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Heading level for markdown symbols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Anchor slug for markdown headings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    /// Whether the symbol is publicly visible.
    #[serde(rename = "public")]
    pub is_public: bool,
}

/// A formatted symbol body row for `kdb symbols -s` output.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolBodyRow {
    /// Path to file containing the symbol, relative to kdb root.
    pub file: String,
    /// Stable kind label used for machine filtering.
    pub kind: String,
    /// Language-native declaration keyword chain.
    pub display_kind: String,
    /// Raw symbol name.
    pub name: String,
    /// 1-based start line number.
    pub line: usize,
    /// 1-based end line number.
    pub end_line: usize,
    /// Parent type/class name for member symbols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Whether the symbol is publicly visible.
    #[serde(rename = "public")]
    pub is_public: bool,
    /// Full source snippet for the symbol declaration/body.
    pub body: String,
}

impl From<Symbol> for SymbolRow {
    fn from(symbol: Symbol) -> Self {
        let display_kind = symbol.display_kind.clone();

        Self {
            display: format_symbol_display(&symbol),
            kind: kind_label(symbol.kind).to_string(),
            display_kind: Some(display_kind),
            name: symbol.name,
            line: symbol.line,
            parent: symbol.parent,
            level: None,
            anchor: None,
            is_public: symbol.is_public,
        }
    }
}

/// Convert a code symbol and source body into a body output row.
pub fn code_symbol_body_row(file: &str, symbol: Symbol, body: String) -> SymbolBodyRow {
    SymbolBodyRow {
        file: file.to_string(),
        kind: kind_label(symbol.kind).to_string(),
        display_kind: symbol.display_kind,
        name: symbol.name,
        line: symbol.line,
        end_line: symbol.end_line,
        parent: symbol.parent,
        is_public: symbol.is_public,
        body,
    }
}

/// Convert a markdown heading and section body into a body output row.
pub fn markdown_symbol_body_row(
    file: &str,
    heading: &Heading,
    end_line: usize,
    body: String,
) -> SymbolBodyRow {
    SymbolBodyRow {
        file: file.to_string(),
        kind: "heading".to_string(),
        display_kind: "#".repeat(usize::from(heading.level)),
        name: heading.title.clone(),
        line: heading.line,
        end_line,
        parent: None,
        is_public: true,
        body,
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

/// Print full symbol bodies as plain text.
pub fn print_bodies_text(rows: &[SymbolBodyRow]) {
    if rows.is_empty() {
        println!("(no symbols)");
        return;
    }

    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            println!();
        }
        print!("{}", row.body);
        if !row.body.ends_with('\n') {
            println!();
        }
    }
}
