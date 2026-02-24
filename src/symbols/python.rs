use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, decorated_parent_or_self, name_from_field, nearest_ancestor, push_symbol,
    walk_depth_first,
};

// --------------------------
// ## Index
//
// fn extract()           L17
// fn is_public_name()    L68
// --------------------------

/// Extract Python classes, methods, and free functions.
pub(super) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    walk_depth_first(root, |node| match node.kind() {
        "class_definition" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let line_node = decorated_parent_or_self(node);
                let is_public = is_public_name(&name);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    line_node,
                    name,
                    None,
                    SymbolKind::Class,
                    is_public,
                );
            }
        }
        "function_definition" => {
            let Some(name) = name_from_field(node, source, "name") else {
                return;
            };
            let parent = nearest_ancestor(node, "class_definition")
                .and_then(|class_node| name_from_field(class_node, source, "name"));
            let kind = if parent.is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let parent_is_public = parent.as_ref().is_none_or(|name| is_public_name(name));
            let is_public = parent_is_public && is_public_name(&name);
            let line_node = decorated_parent_or_self(node);
            push_symbol(
                &mut symbols,
                &mut seen,
                line_node,
                name,
                parent,
                kind,
                is_public,
            );
        }
        _ => {}
    });

    symbols
}

/// Python convention: underscore-prefixed names are treated as non-public.
fn is_public_name(name: &str) -> bool {
    !name.starts_with('_')
}
