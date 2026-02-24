use std::collections::HashSet;
use tree_sitter::Node;

use super::{
    Symbol, SymbolKind, name_from_field, normalized_node_text, push_symbol, walk_depth_first,
};

// -------------------------------------
// src/symbols/typescript.rs
//
// pub(super) fn extract()           L34
// fn member_parent()               L230
// fn declaration_span_node()       L252
// fn variable_span_node()          L267
// fn declaration_display_kind()    L283
// fn class_display_kind()          L307
// fn function_display_kind()       L323
// fn member_display_kind()         L348
// fn property_display_kind()       L365
// fn is_module_level_variable()    L380
// fn declaration_keyword()         L396
// fn signature_until_name()        L423
// fn signature_head()              L446
// fn is_exported()                 L465
// fn has_export_ancestor()         L476
// fn is_default_export()           L490
// fn is_private_member()           L516
// fn normalize_whitespace()        L537
// fn ends_with_token()             L541
// fn contains_token()              L545
// -------------------------------------

/// Extract JavaScript/TypeScript symbols used by code indexing and symbol listing.
pub(super) fn extract(root: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    walk_depth_first(root, |node| match node.kind() {
        "class_declaration" | "abstract_class_declaration" | "class" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                let span_node = declaration_span_node(node);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    span_node,
                    name,
                    None,
                    SymbolKind::Class,
                    class_display_kind(node, source),
                    is_public,
                );
            }
        }
        "interface_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                let span_node = declaration_span_node(node);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    span_node,
                    name,
                    None,
                    SymbolKind::Interface,
                    declaration_display_kind(node, source, "interface", false),
                    is_public,
                );
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                let span_node = declaration_span_node(node);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    span_node,
                    name,
                    None,
                    SymbolKind::TypeAlias,
                    declaration_display_kind(node, source, "type", false),
                    is_public,
                );
            }
        }
        "enum_declaration" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                let span_node = declaration_span_node(node);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    span_node,
                    name,
                    None,
                    SymbolKind::Enum,
                    declaration_display_kind(node, source, "enum", true),
                    is_public,
                );
            }
        }
        "function_declaration" | "generator_function_declaration" | "function_signature" => {
            if let Some(name) = name_from_field(node, source, "name") {
                let is_public = is_exported(node, source);
                let span_node = declaration_span_node(node);
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    span_node,
                    name,
                    None,
                    SymbolKind::Function,
                    function_display_kind(node, source),
                    is_public,
                );
            }
        }
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            let Some(name) = name_from_field(node, source, "name") else {
                return;
            };
            let (parent, parent_is_public) = member_parent(node, source);
            let is_public = parent_is_public && !is_private_member(node, source, &name);

            if name == "constructor" {
                push_symbol(
                    &mut symbols,
                    &mut seen,
                    node,
                    name,
                    parent,
                    SymbolKind::Constructor,
                    "constructor".to_string(),
                    is_public,
                );
                return;
            }

            let display_kind = member_display_kind(node, source, &name);
            let kind = if ends_with_token(&display_kind, "get") {
                SymbolKind::Getter
            } else if ends_with_token(&display_kind, "set") {
                SymbolKind::Setter
            } else {
                SymbolKind::Method
            };

            push_symbol(
                &mut symbols,
                &mut seen,
                node,
                name,
                parent,
                kind,
                display_kind,
                is_public,
            );
        }
        "public_field_definition" | "field_definition" | "property_signature" => {
            let Some(name) = name_from_field(node, source, "name") else {
                return;
            };
            let (parent, parent_is_public) = member_parent(node, source);
            let is_public = parent_is_public && !is_private_member(node, source, &name);
            let display_kind = property_display_kind(node, source, &name);

            push_symbol(
                &mut symbols,
                &mut seen,
                node,
                name,
                parent,
                SymbolKind::Property,
                display_kind,
                is_public,
            );
        }
        "variable_declarator" => {
            if !is_module_level_variable(node) {
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

            let Some(keyword) = declaration_keyword(node, source) else {
                return;
            };
            let is_public = is_exported(node, source);
            let display_kind = if is_public {
                format!("export {keyword}")
            } else {
                keyword.to_string()
            };
            let kind = if keyword == "const" {
                SymbolKind::Const
            } else {
                SymbolKind::Variable
            };
            let span_node = variable_span_node(node);

            push_symbol(
                &mut symbols,
                &mut seen,
                span_node,
                name,
                None,
                kind,
                display_kind,
                is_public,
            );
        }
        _ => {}
    });

    symbols
}

fn member_parent(node: Node<'_>, source: &[u8]) -> (Option<String>, bool) {
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

fn declaration_span_node(node: Node<'_>) -> Node<'_> {
    let mut cursor = node;
    while let Some(parent) = cursor.parent() {
        if parent.kind() == "export_statement" {
            return parent;
        }
        if parent.kind() == "program" {
            break;
        }
        cursor = parent;
    }

    node
}

fn variable_span_node(node: Node<'_>) -> Node<'_> {
    let mut cursor = node;
    let mut declaration = None;
    while let Some(parent) = cursor.parent() {
        match parent.kind() {
            "lexical_declaration" | "variable_declaration" => declaration = Some(parent),
            "export_statement" => return parent,
            "program" => break,
            _ => {}
        }
        cursor = parent;
    }

    declaration.unwrap_or(node)
}

fn declaration_display_kind(
    node: Node<'_>,
    source: &[u8],
    keyword: &str,
    allow_const_prefix: bool,
) -> String {
    let Some(signature) = signature_until_name(node, source) else {
        return keyword.to_string();
    };

    let mut parts = Vec::new();
    if is_exported(node, source) {
        parts.push("export".to_string());
    }
    if contains_token(&signature, "abstract") {
        parts.push("abstract".to_string());
    }
    if allow_const_prefix && contains_token(&signature, "const") {
        parts.push("const".to_string());
    }
    parts.push(keyword.to_string());
    parts.join(" ")
}

fn class_display_kind(node: Node<'_>, source: &[u8]) -> String {
    let Some(signature) = signature_until_name(node, source) else {
        return "class".to_string();
    };

    let mut parts = Vec::new();
    if is_exported(node, source) {
        parts.push("export".to_string());
    }
    if contains_token(&signature, "abstract") {
        parts.push("abstract".to_string());
    }
    parts.push("class".to_string());
    parts.join(" ")
}

fn function_display_kind(node: Node<'_>, source: &[u8]) -> String {
    let Some(signature) = signature_until_name(node, source) else {
        return "function".to_string();
    };
    let normalized = normalize_whitespace(&signature);

    let mut parts = Vec::new();
    if is_exported(node, source) {
        parts.push("export".to_string());
        if is_default_export(node, source) || contains_token(&normalized, "default") {
            parts.push("default".to_string());
        }
    }
    if contains_token(&normalized, "async") {
        parts.push("async".to_string());
    }
    if normalized.contains("function*") || normalized.contains("function *") {
        parts.push("function*".to_string());
    } else {
        parts.push("function".to_string());
    }

    parts.join(" ")
}

fn member_display_kind(node: Node<'_>, source: &[u8], name: &str) -> String {
    let Some(signature) = signature_head(node, source) else {
        return String::new();
    };
    let normalized = normalize_whitespace(&signature);

    let index = normalized.rfind(name).or_else(|| {
        name.strip_prefix('#')
            .and_then(|without_hash| normalized.rfind(without_hash))
    });
    let Some(index) = index else {
        return String::new();
    };

    normalize_whitespace(normalized[..index].trim())
}

fn property_display_kind(node: Node<'_>, source: &[u8], name: &str) -> String {
    if name.starts_with('#') {
        return "#".to_string();
    }

    let Some(signature) = signature_head(node, source) else {
        return String::new();
    };
    let normalized = normalize_whitespace(&signature);
    let Some(index) = normalized.rfind(name) else {
        return String::new();
    };
    normalize_whitespace(normalized[..index].trim())
}

fn is_module_level_variable(node: Node<'_>) -> bool {
    let mut cursor = node;
    while let Some(parent) = cursor.parent() {
        match parent.kind() {
            "program" => return true,
            "export_statement" => return true,
            "statement_block" | "function_declaration" | "method_definition" | "class_body" => {
                return false;
            }
            _ => cursor = parent,
        }
    }

    false
}

fn declaration_keyword(node: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let mut cursor = node;
    while let Some(parent) = cursor.parent() {
        if matches!(
            parent.kind(),
            "lexical_declaration" | "variable_declaration"
        ) {
            let text = parent.utf8_text(source).ok()?.trim_start();
            if text.starts_with("const ") {
                return Some("const");
            }
            if text.starts_with("let ") {
                return Some("let");
            }
            if text.starts_with("var ") {
                return Some("var");
            }
        }
        if parent.kind() == "program" {
            break;
        }
        cursor = parent;
    }

    None
}

fn signature_until_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let head = text
        .split('{')
        .next()
        .unwrap_or(text)
        .split('=')
        .next()
        .unwrap_or(text)
        .split(':')
        .next()
        .unwrap_or(text);
    let normalized = normalize_whitespace(head);

    let name = name_from_field(node, source, "name")?;
    let index = normalized.rfind(&name).or_else(|| {
        name.strip_prefix('#')
            .and_then(|without_hash| normalized.rfind(without_hash))
    })?;

    Some(normalize_whitespace(normalized[..index].trim()))
}

fn signature_head(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let head = text
        .split('{')
        .next()
        .unwrap_or(text)
        .split('=')
        .next()
        .unwrap_or(text)
        .split(':')
        .next()
        .unwrap_or(text)
        .split('(')
        .next()
        .unwrap_or(text);
    Some(normalize_whitespace(head.trim()))
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

fn is_default_export(mut node: Node<'_>, source: &[u8]) -> bool {
    let starts_with_default = node
        .utf8_text(source)
        .ok()
        .map(|text| text.trim_start().starts_with("export default "))
        .unwrap_or(false);
    if starts_with_default {
        return true;
    }

    while let Some(parent) = node.parent() {
        if parent.kind() == "export_statement" {
            let Ok(text) = parent.utf8_text(source) else {
                return false;
            };
            return text.trim_start().starts_with("export default ");
        }
        if parent.kind() == "program" {
            return false;
        }
        node = parent;
    }

    false
}

fn is_private_member(node: Node<'_>, source: &[u8], name: &str) -> bool {
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

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ends_with_token(text: &str, token: &str) -> bool {
    text == token || text.ends_with(&format!(" {token}"))
}

fn contains_token(text: &str, token: &str) -> bool {
    if text == token {
        return true;
    }

    let wrapped = format!(" {text} ");
    wrapped.contains(&format!(" {token} "))
}
