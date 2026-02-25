use tree_sitter::Node;

use super::{
    Extractor, Symbol, SymbolKind, nearest_ancestor, normalize_type_name, walk_depth_first,
};

// -------------------------------------------
// src/symbols/extract/rust.rs
//
// pub(in crate::symbols) fn extract()     L27
// fn method_parent()                     L156
// fn function_is_public()                L170
// fn item_is_public()                    L183
// fn function_display_kind()             L191
// fn trait_display_kind()                L224
// fn item_display_kind()                 L243
// fn static_display_kind()               L259
// fn macro_name()                        L278
// fn declaration_signature()             L296
// fn visibility_prefix()                 L308
// fn leading_visibility()                L315
// fn contains_keyword()                  L350
// fn find_keyword_index()                L355
// -------------------------------------------

/// Extract Rust symbols used by code indexing and symbol listing.
pub(in crate::symbols) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut extractor = Extractor::new(source);

    walk_depth_first(root, |node| match node.kind() {
        "function_item" => {
            let Some(name) = extractor.name_from_field(node, "name") else {
                return;
            };
            let parent = method_parent(node, source, &extractor);
            let kind = if parent.is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let is_public = function_is_public(node, source);
            let display_kind = function_display_kind(node, source);
            extractor.push(node, name, parent, kind, display_kind, is_public);
        }
        "struct_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Struct,
                    item_display_kind(node, source, "struct"),
                    is_public,
                );
            }
        }
        "enum_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Enum,
                    item_display_kind(node, source, "enum"),
                    is_public,
                );
            }
        }
        "trait_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Trait,
                    trait_display_kind(node, source),
                    is_public,
                );
            }
        }
        "type_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::TypeAlias,
                    item_display_kind(node, source, "type"),
                    is_public,
                );
            }
        }
        "const_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Const,
                    item_display_kind(node, source, "const"),
                    is_public,
                );
            }
        }
        "static_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Static,
                    static_display_kind(node, source),
                    is_public,
                );
            }
        }
        "mod_item" => {
            if let Some(name) = extractor.name_from_field(node, "name") {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Module,
                    item_display_kind(node, source, "mod"),
                    is_public,
                );
            }
        }
        "macro_definition" | "macro_rule" => {
            if let Some(name) = macro_name(node, source, &extractor) {
                let is_public = item_is_public(node, source);
                extractor.push(
                    node,
                    name,
                    None,
                    SymbolKind::Macro,
                    "macro_rules!".to_string(),
                    is_public,
                );
            }
        }
        _ => {}
    });

    extractor.finish()
}

/// Resolve the parent type/trait name for a Rust method.
fn method_parent(node: Node<'_>, source: &[u8], extractor: &Extractor<'_>) -> Option<String> {
    if let Some(impl_node) = nearest_ancestor(node, "impl_item") {
        if let Some(type_node) = impl_node.child_by_field_name("type") {
            if let Some(name) = normalize_type_name(type_node, source) {
                return Some(name);
            }
        }
    }

    nearest_ancestor(node, "trait_item")
        .and_then(|trait_node| extractor.name_from_field(trait_node, "name"))
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

fn function_display_kind(node: Node<'_>, source: &[u8]) -> String {
    let Some(signature) = declaration_signature(node, source) else {
        return "fn".to_string();
    };

    let before_fn = find_keyword_index(&signature, "fn")
        .map(|index| signature[..index].trim())
        .unwrap_or(signature.as_str());

    let mut parts = Vec::new();
    if let Some(visibility) = visibility_prefix(node, source)
        .or_else(|| leading_visibility(before_fn))
        .or_else(|| item_is_public(node, source).then(|| "pub".to_string()))
    {
        parts.push(visibility);
    }
    if contains_keyword(before_fn, "const") {
        parts.push("const".to_string());
    }
    if contains_keyword(before_fn, "async") {
        parts.push("async".to_string());
    }
    if contains_keyword(before_fn, "unsafe") {
        parts.push("unsafe".to_string());
    }
    if contains_keyword(before_fn, "extern") {
        parts.push("extern".to_string());
    }
    parts.push("fn".to_string());

    parts.join(" ")
}

fn trait_display_kind(node: Node<'_>, source: &[u8]) -> String {
    let Some(signature) = declaration_signature(node, source) else {
        return "trait".to_string();
    };

    let mut parts = Vec::new();
    if let Some(visibility) = visibility_prefix(node, source)
        .or_else(|| leading_visibility(&signature))
        .or_else(|| item_is_public(node, source).then(|| "pub".to_string()))
    {
        parts.push(visibility);
    }
    if contains_keyword(&signature, "unsafe") {
        parts.push("unsafe".to_string());
    }
    parts.push("trait".to_string());
    parts.join(" ")
}

fn item_display_kind(node: Node<'_>, source: &[u8], item_keyword: &str) -> String {
    let Some(signature) = declaration_signature(node, source) else {
        return item_keyword.to_string();
    };

    let mut parts = Vec::new();
    if let Some(visibility) = visibility_prefix(node, source)
        .or_else(|| leading_visibility(&signature))
        .or_else(|| item_is_public(node, source).then(|| "pub".to_string()))
    {
        parts.push(visibility);
    }
    parts.push(item_keyword.to_string());
    parts.join(" ")
}

fn static_display_kind(node: Node<'_>, source: &[u8]) -> String {
    let Some(signature) = declaration_signature(node, source) else {
        return "static".to_string();
    };

    let mut parts = Vec::new();
    if let Some(visibility) = visibility_prefix(node, source)
        .or_else(|| leading_visibility(&signature))
        .or_else(|| item_is_public(node, source).then(|| "pub".to_string()))
    {
        parts.push(visibility);
    }
    parts.push("static".to_string());
    if contains_keyword(&signature, "mut") {
        parts.push("mut".to_string());
    }
    parts.join(" ")
}

fn macro_name(node: Node<'_>, source: &[u8], extractor: &Extractor<'_>) -> Option<String> {
    if let Some(name) = extractor.name_from_field(node, "name") {
        return Some(name);
    }

    let text = node.utf8_text(source).ok()?.trim_start();
    let rest = text.strip_prefix("macro_rules!")?.trim_start();
    let end = rest
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn declaration_signature(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let without_body = text.split('{').next().unwrap_or(text);
    let without_semicolon = without_body.split(';').next().unwrap_or(without_body);
    let signature = without_semicolon.trim();
    if signature.is_empty() {
        None
    } else {
        Some(signature.to_string())
    }
}

fn visibility_prefix(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("visibility")
        .and_then(|visibility| visibility.utf8_text(source).ok())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn leading_visibility(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("pub") {
        return None;
    }

    let rest = &trimmed[3..];
    if rest.starts_with('(') {
        let mut depth = 0i32;
        for (index, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(trimmed[..3 + index + 1].to_string());
                    }
                }
                _ => {}
            }
        }
        return Some("pub".to_string());
    }

    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_whitespace())
    {
        return Some("pub".to_string());
    }

    None
}

fn contains_keyword(text: &str, keyword: &str) -> bool {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|token| token == keyword)
}

fn find_keyword_index(text: &str, keyword: &str) -> Option<usize> {
    for (index, _) in text.match_indices(keyword) {
        let before = text[..index].chars().next_back();
        let after = text[index + keyword.len()..].chars().next();
        let before_ok = before.is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'));
        let after_ok = after.is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'));
        if before_ok && after_ok {
            return Some(index);
        }
    }

    None
}
