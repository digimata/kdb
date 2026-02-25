//! Language-aware code symbol extraction.

use anyhow::Result;
use std::collections::HashSet;
use tree_sitter::Node;

use crate::lang::CodeLanguage;

pub mod display;
mod extract;
// -----------------------------------------
// src/symbols/mod.rs
//
// pub mod display                        L9
// mod extract                           L10
// pub(crate) mod query                  L30
// mod tree                              L31
// pub enum SymbolKind                   L38
// pub struct Symbol                     L60
// struct SeenSymbolKey                  L73
// pub(super) struct Extractor           L85
//   pub(super) fn new()                 L92
//   pub(super) fn name_from_field()    L100
//   pub(super) fn node_text()          L105
//   pub(super) fn push()               L109
//   pub(super) fn finish()             L148
// pub fn extract_symbols()             L154
// -----------------------------------------

pub(crate) mod query;
mod tree;

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
        let line = node.start_position().row as usize + 1;
        let end_line = node.end_position().row as usize + 1;
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

    pub(super) fn finish(self) -> Vec<Symbol> {
        self.symbols
    }
}

/// Parse source and extract language-appropriate symbols.
pub fn extract_symbols(language: CodeLanguage, source: &str) -> Result<Vec<Symbol>> {
    let root_tree = tree::parse_tree(language, source)?;
    let root = root_tree.root_node();
    let source_bytes = source.as_bytes();

    let symbols = match language {
        CodeLanguage::Rust => extract::extract_rust(root, source_bytes),
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            extract::extract_typescript(root, source_bytes)
        }
        CodeLanguage::Python => extract::extract_python(root, source_bytes),
        CodeLanguage::Go => extract::extract_go(root, source_bytes),
    };

    Ok(symbols)
}
