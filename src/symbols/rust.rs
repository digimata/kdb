use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, name_from_field, nearest_ancestor, normalize_type_name, push_symbol,
    walk_depth_first,
};

// -------------------------------
// ## Index
//
// fn extract()                L19
// fn method_parent()         L100
// fn function_is_public()    L114
// fn item_is_public()        L127
// -------------------------------

/// Extract Rust symbols used by the code index and codemap features.
pub(super) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    walk_depth_first(root, |node| match node.kind() {
        "function_item" => {
            let Some(name) = name_from_field(node, source, "name") else {
                return;
            };
            let parent = method_parent(node, source);
            let kind = if parent.is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let is_public = function_is_public(node, source);
            push_symbol(&mut symbols, &mut seen, node, name, parent, kind, is_public);
        }
        "struct_item" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = item_is_public(node, source);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Struct,
                    is_public,
                );
            }
        }
        "enum_item" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = item_is_public(node, source);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Enum,
                    is_public,
                );
            }
        }
        "trait_item" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = item_is_public(node, source);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Trait,
                    is_public,
                );
            }
        }
        "type_item" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = item_is_public(node, source);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::TypeAlias,
                    is_public,
                );
            }
        }
        _ => {}
    });

    symbols
}

/// Resolve the parent type/trait name for a Rust method.
fn method_parent(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(impl_node) = nearest_ancestor(node, "impl_item") {
        if let Some(type_node) = impl_node.child_by_field_name("type") {
            if let Some(name) = normalize_type_name(type_node, source) {
                return Some(name);
            }
        }
    }

    nearest_ancestor(node, "trait_item")
        .and_then(|trait_node| name_from_field(trait_node, source, "name"))
}

/// Determine whether a Rust function should be considered public.
fn function_is_public(node: Node<'_>, source: &[u8]) -> bool {
    if nearest_ancestor(node, "impl_item").is_some() {
        return item_is_public(node, source);
    }

    if let Some(trait_node) = nearest_ancestor(node, "trait_item") {
        return item_is_public(trait_node, source);
    }

    item_is_public(node, source)
}

/// Determine whether a Rust item text starts with a `pub` visibility marker.
fn item_is_public(node: Node<'_>, source: &[u8]) -> bool {
    let Ok(text) = node.utf8_text(source) else {
        return false;
    };
    let trimmed = text.trim_start();
    trimmed.starts_with("pub ") || trimmed.starts_with("pub(")
}
