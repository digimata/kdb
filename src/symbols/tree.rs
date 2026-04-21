//! Tree-sitter parsing and node utilities for symbol extraction.

use anyhow::{Context, Result};
use tree_sitter::{Language, Node, Parser};

use crate::lang::CodeLanguage;

// ------------------------------------------------
// projects/kdb/src/symbols/tree.rs
//
// pub(crate) fn parse_tree()                   L23
// pub(crate) fn tree_sitter_language()         L36
// pub(super) fn trim_node_text()               L48
// pub(crate) fn raw_node_text()                L67
// pub(crate) fn walk_depth_first()             L73
// pub(super) fn nearest_ancestor()            L100
// pub(super) fn normalize_type_name()         L111
// pub(super) fn extract_go_receiver_type()    L146
// pub(super) fn decorated_parent_or_self()    L182
// ------------------------------------------------

/// Parse source text into a tree-sitter syntax tree.
pub(crate) fn parse_tree(language: CodeLanguage, source: &str) -> Result<tree_sitter::Tree> {
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
pub(crate) fn tree_sitter_language(language: CodeLanguage) -> Language {
    match language {
        CodeLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        CodeLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        CodeLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        CodeLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        CodeLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        CodeLanguage::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

/// Extract the trimmed text content of a tree-sitter node.
pub(super) fn trim_node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
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

/// Extract the trimmed text content of a node without quote-stripping.
pub(crate) fn raw_node_text<'a>(node: Node<'_>, source: &'a [u8]) -> Option<&'a str> {
    let text = node.utf8_text(source).ok()?.trim();
    if text.is_empty() { None } else { Some(text) }
}

/// Walk all nodes in depth-first order and invoke `visit` for each.
pub(crate) fn walk_depth_first(root: Node<'_>, mut visit: impl FnMut(Node<'_>)) {
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
        return trim_node_text(name_node, source);
    }

    let text = trim_node_text(node, source)?;
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
