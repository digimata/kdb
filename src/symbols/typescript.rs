use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, is_js_function_value, name_from_field, normalized_node_text, push_symbol,
    walk_depth_first,
};

// --------------------------------
// ## Index
//
// fn extract()                 L20
// fn method_parent()          L135
// fn is_exported()            L158
// fn has_export_ancestor()    L169
// fn is_private_method()      L184
// --------------------------------

/// Extract JavaScript/TypeScript symbols used by the code index and codemap.
pub(super) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    walk_depth_first(root, |node| match node.kind() {
        "class_declaration" | "abstract_class_declaration" | "class" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Class,
                    is_public,
                );
            }
        }
        "interface_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    None,
                    SymbolKind::Interface,
                    is_public,
                );
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
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
        "function_declaration" | "generator_function_declaration" | "function_signature" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
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
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let (parent, parent_is_public) = method_parent(node, source);
                let is_public = parent_is_public && !is_private_method(node, source);
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
        "variable_declarator" => {
            let Some(value) = node.child_by_field_name("value") else {
                return;
            };
            if !is_js_function_value(value.kind()) {
                return;
            }

            let Some(name_node) = node.child_by_field_name("name") else {
                return;
            };
            if !matches!(
                name_node.kind(),
                "identifier" | "shorthand_property_identifier_pattern"
            ) {
                return;
            }
            let Some(name) = normalized_node_text(name_node, source) else {
                return;
            };
            let is_public = is_exported(node, source);

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
        _ => {}
    });

    symbols
}

/// Resolve the nearest class/interface owner for a method-like node.
fn method_parent(node: Node<'_>, source: &[u8]) -> (Option<String>, bool) {
    let mut cursor = node;
    while let Some(parent) = cursor.parent() {
        match parent.kind() {
            "class_declaration"
            | "abstract_class_declaration"
            | "class"
            | "interface_declaration" => {
                return (
                    name_from_field(parent, source, "name"),
                    is_exported(parent, source),
                );
            }
            _ => {
                cursor = parent;
            }
        }
    }

    (None, false)
}

/// Determine whether a declaration is exported at top level.
fn is_exported(node: Node<'_>, source: &[u8]) -> bool {
    let starts_with_export = node
        .utf8_text(source)
        .ok()
        .map(|text| text.trim_start().starts_with("export "))
        .unwrap_or(false);

    starts_with_export || has_export_ancestor(node)
}

/// Walk ancestors to find an enclosing `export_statement`.
fn has_export_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "export_statement" {
            return true;
        }
        if parent.kind() == "program" {
            return false;
        }
        node = parent;
    }

    false
}

/// Determine whether a class method is private/protected.
fn is_private_method(node: Node<'_>, source: &[u8]) -> bool {
    let Some(name) = node
        .child_by_field_name("name")
        .and_then(|name_node| normalized_node_text(name_node, source))
    else {
        return false;
    };

    if name.starts_with('#') {
        return true;
    }

    let Ok(text) = node.utf8_text(source) else {
        return false;
    };
    let signature = text
        .trim_start()
        .split('{')
        .next()
        .unwrap_or(text)
        .split('(')
        .next()
        .unwrap_or(text)
        .replace('\t', " ");
    let normalized = format!(" {} ", signature.trim());
    normalized.contains(" private ") || normalized.contains(" protected ")
}
