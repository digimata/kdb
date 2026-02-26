use std::collections::HashSet;
use tree_sitter::Node;

use crate::lang::CodeLanguage;

use super::{
    Extractor, Symbol, SymbolKind, nearest_ancestor, normalize_type_name, parse_tree,
    walk_depth_first,
};

// -------------------------------------------
// src/symbols/extract/rust.rs
//
// pub(in crate::symbols) fn extract()     L36
// fn method_parent()                     L170
// fn function_is_public()                L183
// fn item_is_public()                    L196
// fn function_display_kind()             L204
// fn trait_display_kind()                L237
// fn item_display_kind()                 L256
// fn static_display_kind()               L272
// fn macro_name()                        L291
// fn declaration_signature()             L309
// fn visibility_prefix()                 L321
// fn leading_visibility()                L328
// fn contains_keyword()                  L363
// fn find_keyword_index()                L368
// fn extract_from_macro()                L392
// fn strip_outer_delimiters()            L440
// fn try_parse_and_extract()             L457
// fn extract_from_balanced_blocks()      L471
// fn dedup_across_branches()             L512
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
        "macro_invocation" => {
            for sym in extract_from_macro(node, source) {
                extractor.push_raw(sym);
            }
        }
        _ => {}
    });

    extractor.finish()
}

/// Resolve the parent type/trait name for a Rust method.
fn method_parent(node: Node<'_>, source: &[u8], extractor: &Extractor<'_>) -> Option<String> {
    if let Some(impl_node) = nearest_ancestor(node, "impl_item")
        && let Some(type_node) = impl_node.child_by_field_name("type")
        && let Some(name) = normalize_type_name(type_node, source)
    {
        return Some(name);
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

// ------------------------------------------------------------------
// Heuristic macro token-tree re-parsing
// ------------------------------------------------------------------

/// Extract symbols from a `macro_invocation` node by re-parsing its token tree.
///
/// Tree-sitter cannot expand macros, so symbols defined inside `cfg_if!` and
/// similar structural macros are invisible. This function extracts the token
/// tree text, attempts to parse it as Rust, and returns any symbols found with
/// their line/byte offsets adjusted to the original source.
fn extract_from_macro(node: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    let Some(token_tree) = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "token_tree")
    else {
        return Vec::new();
    };

    let Some(raw) = token_tree.utf8_text(source).ok() else {
        return Vec::new();
    };

    // Strip the outermost delimiter pair.
    let content = strip_outer_delimiters(raw);
    if content.trim().is_empty() {
        return Vec::new();
    }

    // Byte offset of the content within the original source.
    let delim_offset = if raw.len() > content.len() { 1 } else { 0 };
    let content_start_byte = token_tree.start_byte() + delim_offset;
    let content_start_line = token_tree.start_position().row;
    // Count how many newlines precede `content` within `raw`.
    let prefix = &raw[..delim_offset];
    let prefix_newlines = prefix.chars().filter(|&c| c == '\n').count();
    let line_offset = content_start_line + prefix_newlines;

    // First attempt: parse the full stripped content as Rust.
    let mut symbols = try_parse_and_extract(content);

    // Fallback: scan for balanced `{ }` blocks at depth 0 and try each.
    if symbols.is_empty() {
        symbols = extract_from_balanced_blocks(content);
    }

    // Offset all symbols back to original-source coordinates.
    for sym in &mut symbols {
        sym.line += line_offset;
        sym.end_line += line_offset;
        sym.start_byte += content_start_byte;
        sym.end_byte += content_start_byte;
    }

    // Deduplicate across cfg branches: keep first per (name, kind, parent).
    dedup_across_branches(&mut symbols);

    symbols
}

/// Strip the outermost `{}`, `[]`, or `()` from a token tree string.
fn strip_outer_delimiters(text: &str) -> &str {
    let trimmed = text.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 {
        return trimmed;
    }

    let (open, close) = (bytes[0], bytes[bytes.len() - 1]);
    let matched = matches!((open, close), (b'{', b'}') | (b'[', b']') | (b'(', b')'));
    if matched {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

/// Try to parse `content` as Rust and extract symbols.
fn try_parse_and_extract(content: &str) -> Vec<Symbol> {
    let Ok(tree) = parse_tree(CodeLanguage::Rust, content) else {
        return Vec::new();
    };

    let root = tree.root_node();
    if root.has_error() {
        return Vec::new();
    }

    extract(root, content.as_bytes())
}

/// Scan for balanced `{ ... }` blocks at depth 0, extract each as Rust.
fn extract_from_balanced_blocks(content: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut depth = 0i32;
    let mut block_start: Option<usize> = None;

    for (i, ch) in content.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    block_start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0
                    && let Some(start) = block_start.take()
                {
                    let inner = &content[start + 1..i];
                    let mut block_syms = try_parse_and_extract(inner);
                    // Offset by the block's position within content.
                    let newlines_before =
                        content[..start + 1].chars().filter(|&c| c == '\n').count();
                    for sym in &mut block_syms {
                        sym.line += newlines_before;
                        sym.end_line += newlines_before;
                        sym.start_byte += start + 1;
                        sym.end_byte += start + 1;
                    }
                    symbols.extend(block_syms);
                }
            }
            _ => {}
        }
    }

    symbols
}

/// Deduplicate symbols from multiple cfg branches, keeping the first occurrence
/// per `(name, kind, parent)` tuple.
fn dedup_across_branches(symbols: &mut Vec<Symbol>) {
    let mut seen: HashSet<(String, SymbolKind, Option<String>)> = HashSet::new();
    symbols.retain(|sym| seen.insert((sym.name.clone(), sym.kind, sym.parent.clone())));
}
