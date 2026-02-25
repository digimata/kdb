//! Symbol display formatting, CLI output, and codemap generation.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::index::Heading;

use super::{Symbol, SymbolKind};

// -----------------------------------------
// src/symbols/display.rs
//
// pub struct SymbolRow                  L28
//   fn from()                           L56
// pub struct SymbolBodyRow              L75
// pub fn kind_label()                   L99
// pub fn is_callable_kind()            L120
// pub fn format_symbol_display()       L132
// pub fn extract_symbol_body()         L160
// pub fn code_symbol_body_row()        L173
// pub fn markdown_symbol_body_row()    L188
// pub fn print_text()                  L208
// pub fn print_bodies_text()           L221
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

/// Stable symbol kind labels for JSON output and filtering.
pub fn kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor => "fn",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::TypeAlias => "type",
        SymbolKind::Class => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::Const => "const",
        SymbolKind::Static => "static",
        SymbolKind::Property => "property",
        SymbolKind::Getter => "getter",
        SymbolKind::Setter => "setter",
        SymbolKind::Module => "module",
        SymbolKind::Macro => "macro",
        SymbolKind::Variable => "variable",
    }
}

/// Return whether a symbol should be displayed as callable with trailing `()`.
pub fn is_callable_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Function
            | SymbolKind::Method
            | SymbolKind::Constructor
            | SymbolKind::Getter
            | SymbolKind::Setter
    )
}

/// Build a display line fragment for a code symbol.
pub fn format_symbol_display(symbol: &Symbol) -> String {
    let indent = if symbol.parent.is_some() { "  " } else { "" };
    let display_kind = symbol.display_kind.trim();

    if symbol.kind == SymbolKind::Constructor
        && display_kind == "constructor"
        && symbol.name == "constructor"
    {
        return format!("{indent}constructor()");
    }

    let mut display_name = symbol.name.clone();
    if is_callable_kind(symbol.kind) {
        display_name.push_str("()");
    }

    if display_kind == "#" {
        return format!("{indent}#{}", display_name.trim_start_matches('#'));
    }

    if display_kind.is_empty() {
        format!("{indent}{display_name}")
    } else {
        format!("{indent}{display_kind} {display_name}")
    }
}

/// Extract the source body for a symbol using byte span coordinates.
pub fn extract_symbol_body(source: &str, symbol: &Symbol) -> Result<String> {
    source
        .get(symbol.start_byte..symbol.end_byte)
        .map(|slice| slice.to_string())
        .with_context(|| {
            format!(
                "invalid symbol span {}..{} for `{}`",
                symbol.start_byte, symbol.end_byte, symbol.name
            )
        })
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
