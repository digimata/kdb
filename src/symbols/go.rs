use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, extract_go_receiver_type, name_from_field, push_symbol, walk_depth_first,
};

// ----------------------------
// src/symbols/go.rs
//
// fn extract()             L17
// fn receiver_parent()     L73
// fn is_exported_name()    L80
// ----------------------------

/// Extract Go functions, methods, and named type declarations.
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
                    is_public,
                );
            }
        }
        "type_spec" => {
            let Some(name) = name_from_field(node, source, "name") else {
                return;
            };
            let kind = node
                .child_by_field_name("type")
                .map(|type_node| match type_node.kind() {
                    "struct_type" => SymbolKind::Struct,
                    "interface_type" => SymbolKind::Interface,
                    _ => SymbolKind::TypeAlias,
                })
                .unwrap_or(SymbolKind::TypeAlias);
            let is_public = is_exported_name(&name);
            push_symbol(&mut symbols, &mut seen, node, name, None, kind, is_public);
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

/// Go convention: exported identifiers begin with an uppercase letter.
fn is_exported_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}
