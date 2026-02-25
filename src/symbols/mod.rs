//! Language-aware code symbol extraction.

use anyhow::{Context, Result};
use std::collections::HashSet;
use tree_sitter::{Language, Node, Parser};

use crate::lang::CodeLanguage;

mod go;
mod python;
// ------------------------------------------------
// src/symbols/mod.rs
//
// mod go                                        L9
// mod python                                   L10
// pub(crate) mod query                         L40
// pub mod render                               L41
// mod rust                                     L42
// mod typescript                               L43
// pub enum SymbolKind                          L47
// pub struct Symbol                            L69
// type SeenSymbols                             L81
// pub fn extract_symbols()                     L93
// fn parse_tree()                             L111
// fn tree_sitter_language()                   L124
// pub(super) fn walk_depth_first()            L136
// pub(super) fn name_from_field()             L163
// pub(super) fn normalized_node_text()        L169
// pub(super) fn push_symbol()                 L188
// pub fn extract_symbol_body()                L228
// pub(super) fn nearest_ancestor()            L241
// pub(super) fn normalize_type_name()         L252
// pub(super) fn extract_go_receiver_type()    L287
// pub(super) fn decorated_parent_or_self()    L323
// pub fn kind_label()                         L331
// pub fn is_callable_kind()                   L352
// pub fn format_symbol_display()              L364
// ------------------------------------------------

pub(crate) mod query;
pub mod render;
mod rust;
mod typescript;

/// Symbol categories rendered by `kdb fmt` and `kdb codemap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Class,
    Interface,
    Const,
    Static,
    Property,
    Getter,
    Setter,
    Module,
    Macro,
    Constructor,
    Variable,
}

/// A declaration extracted from a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub parent: Option<String>,
    pub kind: SymbolKind,
    pub display_kind: String,
    pub line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub is_public: bool,
}

type SeenSymbols = HashSet<(
    usize,
    usize,
    usize,
    String,
    Option<String>,
    SymbolKind,
    String,
    bool,
)>;

/// Parse source and extract language-appropriate symbols.
pub fn extract_symbols(language: CodeLanguage, source: &str) -> Result<Vec<Symbol>> {
    let tree = parse_tree(language, source)?;
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    let symbols = match language {
        CodeLanguage::Rust => rust::extract(root, source_bytes),
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            typescript::extract(root, source_bytes)
        }
        CodeLanguage::Python => python::extract(root, source_bytes),
        CodeLanguage::Go => go::extract(root, source_bytes),
    };

    Ok(symbols)
}

/// Parse source text into a tree-sitter syntax tree.
fn parse_tree(language: CodeLanguage, source: &str) -> Result<tree_sitter::Tree> {
    let mut parser = Parser::new();
    let ts_language = tree_sitter_language(language);
    parser
        .set_language(&ts_language)
        .with_context(|| format!("failed to load {} parser", language.as_str()))?;

    parser
        .parse(source, None)
        .context("tree-sitter failed to parse source")
}

/// Map a language enum to its tree-sitter grammar.
fn tree_sitter_language(language: CodeLanguage) -> Language {
    match language {
        CodeLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        CodeLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        CodeLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        CodeLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        CodeLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        CodeLanguage::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

/// Walk all nodes in depth-first order and invoke `visit` for each.
pub(super) fn walk_depth_first(root: Node<'_>, mut visit: impl FnMut(Node<'_>)) {
    let mut cursor = root.walk();

    loop {
        let node = cursor.node();
        visit(node);

        if cursor.goto_first_child() {
            continue;
        }

        if cursor.goto_next_sibling() {
            continue;
        }

        loop {
            if !cursor.goto_parent() {
                return;
            }
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Read a named child field and return normalized text.
pub(super) fn name_from_field(node: Node<'_>, source: &[u8], field: &str) -> Option<String> {
    let name_node = node.child_by_field_name(field)?;
    normalized_node_text(name_node, source)
}

/// Normalize a node's text by trimming whitespace and quote wrappers.
pub(super) fn normalized_node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?.trim();
    if text.is_empty() {
        return None;
    }

    let stripped = text
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
        .trim();
    if stripped.is_empty() {
        return None;
    }

    Some(stripped.to_string())
}

/// Add a symbol with deduplication by location/name/kind/visibility.
pub(super) fn push_symbol(
    symbols: &mut Vec<Symbol>,
    seen: &mut SeenSymbols,
    node: Node<'_>,
    name: String,
    parent: Option<String>,
    kind: SymbolKind,
    display_kind: String,
    is_public: bool,
) {
    let line = node.start_position().row as usize + 1;
    let end_line = node.end_position().row as usize + 1;
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();
    let key = (
        line,
        end_line,
        start_byte,
        name.clone(),
        parent.clone(),
        kind,
        display_kind.clone(),
        is_public,
    );
    if seen.insert(key) {
        symbols.push(Symbol {
            name,
            parent,
            kind,
            display_kind,
            line,
            end_line,
            start_byte,
            end_byte,
            is_public,
        });
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

/// Find the nearest ancestor node of a given kind.
pub(super) fn nearest_ancestor<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

/// Normalize a Rust type node to a display name used for method qualification.
pub(super) fn normalize_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return normalized_node_text(name_node, source);
    }

    let text = normalized_node_text(node, source)?;
    let path_segment = text.split("::").last().unwrap_or(&text);
    let path_segment = path_segment.split('.').last().unwrap_or(path_segment);
    let path_segment = path_segment.trim();

    let path_segment = path_segment.trim_start_matches('&').trim_start_matches('*');
    let path_segment = path_segment
        .trim_start_matches("mut ")
        .trim_start_matches("const ")
        .trim();
    let path_segment = path_segment
        .split('<')
        .next()
        .unwrap_or(path_segment)
        .trim();
    let path_segment = path_segment
        .trim_matches('(')
        .trim_matches(')')
        .trim_matches('[')
        .trim_matches(']')
        .trim();

    if path_segment.is_empty() {
        None
    } else {
        Some(path_segment.to_string())
    }
}

/// Parse a Go receiver declaration and return the receiver type name.
pub(super) fn extract_go_receiver_type(text: &str) -> Option<String> {
    let mut stripped = String::new();
    let mut generic_depth = 0i32;
    for ch in text.chars() {
        match ch {
            '[' => generic_depth += 1,
            ']' => generic_depth -= 1,
            _ if generic_depth <= 0 => stripped.push(ch),
            _ => {}
        }
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in stripped.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
            continue;
        }

        if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.len() < 2 {
        return None;
    }

    tokens.last().cloned()
}

/// For decorated Python defs, point at the decorator line instead of `def`.
pub(super) fn decorated_parent_or_self(node: Node<'_>) -> Node<'_> {
    match node.parent() {
        Some(parent) if parent.kind() == "decorated_definition" => parent,
        _ => node,
    }
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
