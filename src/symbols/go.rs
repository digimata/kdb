use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, extract_go_receiver_type, name_from_field, normalized_node_text,
    push_symbol, walk_depth_first,
};

/// Extract Go functions, methods, named types, and top-level const/var symbols.
pub(super) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    walk_depth_first(root, |node| match node.kind() {
        "function_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported_name(&name);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Function,
                    "func".to_string(),
                    is_public,
                );
            }
        }
        "method_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let parent = receiver_parent(node, source);
                let is_public = is_exported_name(&name);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    parent,
                    SymbolKind::Method,
                    "func".to_string(),
                    is_public,
                );
            }
        }
        "type_spec" => {
            let Some(name) = name_from_field(node, source, "name") else {
                return;
            };
            let (kind, display_kind) = node
                .child_by_field_name("type")
                .map(|type_node| match type_node.kind() {
                    "struct_type" => (SymbolKind::Struct, "type struct".to_string()),
                    "interface_type" => (SymbolKind::Interface, "type interface".to_string()),
                    _ => (SymbolKind::TypeAlias, "type".to_string()),
                })
                .unwrap_or((SymbolKind::TypeAlias, "type".to_string()));
            let is_public = is_exported_name(&name);
            push_symbol(
                &mut symbols,
                &mut seen,
                node,
                name,
                None,
                kind,
                display_kind,
                is_public,
            );
        }
        "const_spec" => {
            if !is_top_level_spec(node) {
                return;
            }
            for name in names_from_spec(node, source) {
                let is_public = is_exported_name(&name);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Const,
                    "const".to_string(),
                    is_public,
                );
            }
        }
        "var_spec" => {
            if !is_top_level_spec(node) {
                return;
            }
            for name in names_from_spec(node, source) {
                let is_public = is_exported_name(&name);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Variable,
                    "var".to_string(),
                    is_public,
                );
            }
        }
        _ => {}
    });

    symbols
}

/// Extract the receiver type name from a Go method declaration.
fn receiver_parent(node: Node<'_>, source: &[u8]) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let text = receiver.utf8_text(source).ok()?;
    extract_go_receiver_type(text)
}

fn is_top_level_spec(node: Node<'_>) -> bool {
    let mut cursor = node;
    while let Some(parent) = cursor.parent() {
        match parent.kind() {
            "source_file" => return true,
            "block" | "function_declaration" | "method_declaration" | "func_literal" => {
                return false;
            }
            _ => cursor = parent,
        }
    }

    false
}

fn names_from_spec(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(name_node) = node.child_by_field_name("name") {
        if let Some(name) = normalized_node_text(name_node, source) {
            names.push(name);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "identifier" {
            continue;
        }
        if let Some(name) = normalized_node_text(child, source) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }

    names
}

/// Go convention: exported identifiers begin with an uppercase letter.
fn is_exported_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}
