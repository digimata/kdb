use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, decorated_parent_or_self, name_from_field, nearest_ancestor,
    normalized_node_text, push_symbol, walk_depth_first,
};

/// Extract Python classes, methods, and module-level functions.
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
                    "class".to_string(),
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
            let parent_is_public = parent.as_ref().is_none_or(|value| is_public_name(value));
            let is_public = parent_is_public && is_public_name(&name);
            let line_node = decorated_parent_or_self(node);

            let decorators = decorators_for(node, source);
            let (kind, display_kind) =
                function_kind_and_display(node, source, &name, parent.is_some(), &decorators);

            push_symbol(
                &mut symbols,
                &mut seen,
                line_node,
                name,
                parent,
                kind,
                display_kind,
                is_public,
            );
        }
        _ => {}
    });

    symbols
}

fn function_kind_and_display(
    node: Node<'_>,
    source: &[u8],
    name: &str,
    has_parent: bool,
    decorators: &[String],
) -> (SymbolKind, String) {
    if has_parent && name == "__init__" {
        return (SymbolKind::Constructor, "def".to_string());
    }

    if decorators
        .iter()
        .any(|decorator| decorator.ends_with(".setter"))
    {
        return (SymbolKind::Setter, "@setter def".to_string());
    }
    if decorators.iter().any(|decorator| decorator == "property") {
        return (SymbolKind::Getter, "@property def".to_string());
    }
    if decorators
        .iter()
        .any(|decorator| decorator == "staticmethod")
    {
        return (
            symbol_kind_for_context(has_parent),
            "@staticmethod def".to_string(),
        );
    }
    if decorators
        .iter()
        .any(|decorator| decorator == "classmethod")
    {
        return (
            symbol_kind_for_context(has_parent),
            "@classmethod def".to_string(),
        );
    }
    if decorators
        .iter()
        .any(|decorator| decorator == "abstractmethod")
    {
        return (
            symbol_kind_for_context(has_parent),
            "@abstractmethod def".to_string(),
        );
    }

    let base = if is_async_function(node, source) {
        "async def"
    } else {
        "def"
    };
    (symbol_kind_for_context(has_parent), base.to_string())
}

fn symbol_kind_for_context(has_parent: bool) -> SymbolKind {
    if has_parent {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    }
}

fn decorators_for(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let Some(parent) = node.parent() else {
        return Vec::new();
    };
    if parent.kind() != "decorated_definition" {
        return Vec::new();
    }

    let mut decorators = Vec::new();
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() != "decorator" {
            continue;
        }
        if let Some(name_node) = child.child_by_field_name("name") {
            if let Some(name) = normalized_node_text(name_node, source) {
                decorators.push(name);
                continue;
            }
        }

        if let Ok(text) = child.utf8_text(source) {
            let normalized = text.trim_start_matches('@').trim();
            if !normalized.is_empty() {
                decorators.push(normalized.to_string());
            }
        }
    }

    decorators
}

fn is_async_function(node: Node<'_>, source: &[u8]) -> bool {
    let Ok(text) = node.utf8_text(source) else {
        return false;
    };
    text.trim_start().starts_with("async def ")
}

/// Python convention: underscore-prefixed names are treated as non-public.
fn is_public_name(name: &str) -> bool {
    !name.starts_with('_')
}
