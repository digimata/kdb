//! Semantic token support for generated index blocks.
//!
//! This module is intentionally minimal for now and will be expanded in
//! `iss-0017` to colorize `## Index` header rows in supported editors.

// --------------------------------
// ## Index
//
// fn is_index_header_line()    L14
// --------------------------------

/// Return `true` if a line looks like the start of a generated code index block.
#[allow(dead_code)]
pub(crate) fn is_index_header_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("// ## Index") || trimmed.starts_with("# ## Index")
}
