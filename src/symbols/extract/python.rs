use tree_sitter::Node;

use super::{
    Extractor, Symbol, SymbolKind, decorated_parent_or_self, nearest_ancestor, walk_depth_first,
};

// -------------------------------------------
// qmd/src/symbols/extract/python.rs
//
// pub(in crate::symbols) fn extract()     L19
// fn function_kind_and_display()          L59
// fn symbol_kind_for_context()           L115
// fn decorators_for()                    L123
// fn is_async_function()                 L155
// fn is_public_name()                    L163
// -------------------------------------------

/// Extract Python classes, methods, and module-level functions.
pub(in crate::symbols) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut extractor = Extractor::new(source);

    walk_depth_first(root, |node| match node.kind() {
        "class_definition" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let line_node = decorated_parent_or_self(node);
                let is_public = is_public_name(&name);
                extractor.push(
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
            let Some(name) = extractor.name_from_field(node, "name") else {
                return;
            };
            let parent = nearest_ancestor(node, "class_definition")
                .and_then(|class_node| extractor.name_from_field(class_node, "name"));
            let parent_is_public = parent.as_ref().is_none_or(|value| is_public_name(value));
            let is_public = parent_is_public && is_public_name(&name);
            let line_node = decorated_parent_or_self(node);

            let decorators = decorators_for(node, source, &extractor);
            let (kind, display_kind) =
                function_kind_and_display(node, source, &name, parent.is_some(), &decorators);

            extractor.push(line_node, name, parent, kind, display_kind, is_public);
        }
        _ => {}
    });

    extractor.finish()
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

fn decorators_for(node: Node<'_>, source: &[u8], extractor: &Extractor<'_>) -> Vec<String> {
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
            if let Some(name) = extractor.node_text(name_node) {
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
