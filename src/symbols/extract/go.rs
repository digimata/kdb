use tree_sitter::Node;

use super::{Extractor, Symbol, SymbolKind, extract_go_receiver_type, walk_depth_first};

// -------------------------------------------
// qmd/src/symbols/extract/go.rs
//
// pub(in crate::symbols) fn extract()     L16
// fn receiver_parent()                   L101
// fn is_top_level_spec()                 L107
// fn names_from_spec()                   L122
// fn is_exported_name()                  L147
// -------------------------------------------

/// Extract Go functions, methods, named types, and top-level const/var symbols.
pub(in crate::symbols) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut extractor = Extractor::new(source);

    walk_depth_first(root, |node| match node.kind() {
        "function_declaration" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = is_exported_name(&name);
                extractor.push(
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
            if let Some(name) = extractor.name_from_field(node, "name") {
                let parent = receiver_parent(node, source);
                let is_public = is_exported_name(&name);
                extractor.push(
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
            let Some(name) = extractor.name_from_field(node, "name") else {
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
            extractor.push(node, name, None, kind, display_kind, is_public);
        }
        "const_spec" => {
            if !is_top_level_spec(node) {
                return;
            }
            for name in names_from_spec(node, &extractor) {
                let is_public = is_exported_name(&name);
                extractor.push(
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
            for name in names_from_spec(node, &extractor) {
                let is_public = is_exported_name(&name);
                extractor.push(
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

    extractor.finish()
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

fn names_from_spec(node: Node<'_>, extractor: &Extractor<'_>) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(name_node) = node.child_by_field_name("name") {
        if let Some(name) = extractor.node_text(name_node) {
            names.push(name);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "identifier" {
            continue;
        }
        if let Some(name) = extractor.node_text(child) {
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
