//! Semantic token support for generated index blocks.
//!
//! This module is intentionally minimal for now and will be expanded in
//! `iss-0017` to colorize index header rows in supported editors.

// -------------------------------------------
// src/lsp/semantic_tokens.rs
//
// pub(crate) fn is_index_header_line()    L14
// -------------------------------------------

/// Return `true` if a line looks like the start of a generated code index block.
#[allow(dead_code)]
pub(crate) fn is_index_header_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed
        .strip_prefix("// ")
        .or_else(|| trimmed.strip_prefix("# "))
    else {
        return false;
    };

    let value = rest.trim();
    if value == "## Index" {
        return true;
    }

    !value.is_empty()
        && !value.contains(' ')
        && value
            .rsplit('/')
            .next()
            .and_then(|name| name.rsplit_once('.'))
            .is_some_and(|(_, ext)| {
                !ext.is_empty() && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
            })
}
