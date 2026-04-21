//! Language-aware code symbol extraction.

use anyhow::Result;
use std::collections::HashSet;
use tree_sitter::Node;

use crate::lang::CodeLanguage;

pub mod display;
mod extract;
// ------------------------------------------
// projects/kdb/src/symbols/mod.rs
//
// pub mod display                         L9
// mod extract                            L10
// pub(crate) mod query                   L32
// mod tree                               L33
// pub enum SymbolKind                    L43
// pub struct Symbol                      L65
// struct SeenSymbolKey                   L78
// pub(super) struct Extractor            L90
//   pub(super) fn new()                  L97
//   pub(super) fn name_from_field()     L105
//   pub(super) fn node_text()           L110
//   pub(super) fn push()                L114
//   pub(super) fn push_raw()            L154
//   pub(super) fn finish()              L171
// pub fn extract_symbols()              L180
// pub fn extract_symbols_from_tree()    L189
// ------------------------------------------

pub(crate) mod query;
mod tree;

// Re-export tree-sitter helpers for use outside the symbols module.
pub(crate) use tree::{parse_tree, raw_node_text, walk_depth_first};

// Re-export display utilities for external callers.
pub use display::{extract_symbol_body, format_symbol_display, is_callable_kind, kind_label};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SeenSymbolKey {
    line: usize,
    end_line: usize,
    start_byte: usize,
    name: String,
    parent: Option<String>,
    kind: SymbolKind,
    display_kind: String,
    is_public: bool,
}

/// Shared context for per-language symbol extractors.
pub(super) struct Extractor<'src> {
    source: &'src [u8],
    symbols: Vec<Symbol>,
    seen: HashSet<SeenSymbolKey>,
}

impl<'src> Extractor<'src> {
    pub(super) fn new(source: &'src [u8]) -> Self {
        Self {
            source,
            symbols: Vec::new(),
            seen: HashSet::new(),
        }
    }

    pub(super) fn name_from_field(&self, node: Node<'_>, field: &str) -> Option<String> {
        let name_node = node.child_by_field_name(field)?;
        self.node_text(name_node)
    }

    pub(super) fn node_text(&self, node: Node<'_>) -> Option<String> {
        tree::trim_node_text(node, self.source)
    }

    pub(super) fn push(
        &mut self,
        node: Node<'_>,
        name: String,
        parent: Option<String>,
        kind: SymbolKind,
        display_kind: String,
        is_public: bool,
    ) {
        let line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let key = SeenSymbolKey {
            line,
            end_line,
            start_byte,
            name: name.clone(),
            parent: parent.clone(),
            kind,
            display_kind: display_kind.clone(),
            is_public,
        };

        if self.seen.insert(key) {
            self.symbols.push(Symbol {
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

    /// Push a pre-built symbol, deduplicating by the same key as [`push`].
    pub(super) fn push_raw(&mut self, symbol: Symbol) {
        let key = SeenSymbolKey {
            line: symbol.line,
            end_line: symbol.end_line,
            start_byte: symbol.start_byte,
            name: symbol.name.clone(),
            parent: symbol.parent.clone(),
            kind: symbol.kind,
            display_kind: symbol.display_kind.clone(),
            is_public: symbol.is_public,
        };

        if self.seen.insert(key) {
            self.symbols.push(symbol);
        }
    }

    pub(super) fn finish(self) -> Vec<Symbol> {
        self.symbols
    }
}

/// Parse source and extract language-appropriate symbols.
///
/// Convenience wrapper that parses internally. Prefer
/// [`extract_symbols_from_tree`] when a pre-parsed tree is available.
pub fn extract_symbols(language: CodeLanguage, source: &str) -> Result<Vec<Symbol>> {
    let tree = tree::parse_tree(language, source)?;
    Ok(extract_symbols_from_tree(language, source, &tree))
}

/// Extract language-appropriate symbols from a pre-parsed tree-sitter tree.
///
/// The `tree` must have been parsed from `source` — callers parse once and
/// pass both to avoid redundant parsing.
pub fn extract_symbols_from_tree(
    language: CodeLanguage,
    source: &str,
    tree: &tree_sitter::Tree,
) -> Vec<Symbol> {
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    match language {
        CodeLanguage::Rust => extract::extract_rust(root, source_bytes),
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            extract::extract_typescript(root, source_bytes)
        }
        CodeLanguage::Python => extract::extract_python(root, source_bytes),
        CodeLanguage::Go => extract::extract_go(root, source_bytes),
    }
}
