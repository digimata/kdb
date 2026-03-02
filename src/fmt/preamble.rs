//! Preamble detection helpers used by `kdb fmt`.

use crate::lang::CodeLanguage;

// -------------------------------------------
// kdb/src/fmt/preamble.rs
//
// pub fn comment_prefix()                 L31
// pub fn preamble_end_index()             L44
// fn rust_preamble_end()                  L57
// fn ts_js_preamble_end()                L119
// fn python_preamble_end()               L159
// fn go_preamble_end()                   L201
// fn consume_c_block()                   L256
// fn consume_go_import()                 L269
// fn consume_statement()                 L288
// fn consume_python_docstring()          L316
// fn python_docstring_delimiter()        L347
// fn is_rust_use_statement()             L367
// fn is_rust_mod_declaration()           L371
// fn is_rust_extern_crate_statement()    L375
// fn is_ts_js_import_statement()         L379
// fn is_ts_js_require_statement()        L383
// fn is_ts_js_export_from_statement()    L389
// fn is_python_import_statement()        L393
// fn count_char()                        L397
// pub fn markdown_preamble_end()         L405
// -------------------------------------------

/// Return the single-line comment prefix for the given language.
pub fn comment_prefix(language: CodeLanguage) -> &'static str {
    match language {
        CodeLanguage::Python => "#",
        CodeLanguage::Rust
        | CodeLanguage::JavaScript
        | CodeLanguage::TypeScript
        | CodeLanguage::Tsx
        | CodeLanguage::Go => "//",
    }
}

/// Return the line index where the file's preamble (comments, imports, etc.)
/// ends and the main body begins. The index block is inserted at this point.
pub fn preamble_end_index(language: CodeLanguage, lines: &[String]) -> usize {
    match language {
        CodeLanguage::Rust => rust_preamble_end(lines),
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            ts_js_preamble_end(lines)
        }
        CodeLanguage::Python => python_preamble_end(lines),
        CodeLanguage::Go => go_preamble_end(lines),
    }
}

/// Find the end of the Rust preamble (inner attributes, `use`/`extern crate`
/// statements, `mod` declarations, comments, and blank lines).
fn rust_preamble_end(lines: &[String]) -> usize {
    let mut index = 0usize;
    let mut in_block_comment = false;

    while index < lines.len() {
        let trimmed = lines[index].trim();

        if in_block_comment {
            index += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if trimmed.starts_with("//!") {
            index += 1;
            continue;
        }
        if trimmed.starts_with("///") {
            break;
        }
        if trimmed.starts_with("//") {
            index += 1;
            continue;
        }
        if trimmed.starts_with("/*") {
            if trimmed.starts_with("/**") {
                break;
            }
            in_block_comment = !trimmed.contains("*/");
            index += 1;
            continue;
        }
        if trimmed.starts_with("#![") {
            index += 1;
            continue;
        }
        if is_rust_use_statement(trimmed) || is_rust_extern_crate_statement(trimmed) {
            index = consume_statement(lines, index);
            continue;
        }
        if is_rust_mod_declaration(trimmed) {
            if trimmed.contains(';') {
                index += 1;
                continue;
            }
            break;
        }

        break;
    }

    index
}

/// Find the end of the JS/TS preamble (import/require/export-from statements,
/// comments, and blank lines).
fn ts_js_preamble_end(lines: &[String]) -> usize {
    let mut index = 0usize;
    let mut in_block_comment = false;

    while index < lines.len() {
        let trimmed = lines[index].trim();

        if in_block_comment {
            index += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with("//") {
            index += 1;
            continue;
        }
        if trimmed.starts_with("/*") {
            in_block_comment = !trimmed.contains("*/");
            index += 1;
            continue;
        }
        if is_ts_js_import_statement(trimmed)
            || is_ts_js_require_statement(trimmed)
            || is_ts_js_export_from_statement(trimmed)
        {
            index = consume_statement(lines, index);
            continue;
        }

        break;
    }

    index
}

/// Find the end of the Python preamble (shebang, module docstring, comments,
/// and import/from-import statements).
fn python_preamble_end(lines: &[String]) -> usize {
    let mut index = 0usize;

    if lines
        .first()
        .is_some_and(|line| line.trim_start().starts_with("#!"))
    {
        index = 1;
    }

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            index += 1;
            continue;
        }
        break;
    }

    if let Some(next_index) = consume_python_docstring(lines, index) {
        index = next_index;
    }

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            index += 1;
            continue;
        }
        if is_python_import_statement(trimmed) {
            index = consume_statement(lines, index);
            continue;
        }

        break;
    }

    index
}

/// Find the end of the Go preamble (package declaration, import blocks,
/// comments, and blank lines).
fn go_preamble_end(lines: &[String]) -> usize {
    let mut index = 0usize;
    let mut in_block_comment = false;

    while index < lines.len() {
        let trimmed = lines[index].trim();

        if in_block_comment {
            index += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with("//") {
            index += 1;
            continue;
        }
        if trimmed.starts_with("/*") {
            in_block_comment = !trimmed.contains("*/");
            index += 1;
            continue;
        }
        if trimmed.starts_with("package ") {
            index += 1;
            break;
        }

        break;
    }

    while index < lines.len() {
        let trimmed = lines[index].trim();

        if trimmed.is_empty() || trimmed.starts_with("//") {
            index += 1;
            continue;
        }
        if trimmed.starts_with("/*") {
            index = consume_c_block(lines, index);
            continue;
        }
        if trimmed.starts_with("import ") || trimmed == "import(" || trimmed == "import (" {
            index = consume_go_import(lines, index);
            continue;
        }

        break;
    }

    index
}

/// Advance past a C-style `/* ... */` block comment, returning the next index.
fn consume_c_block(lines: &[String], start: usize) -> usize {
    let mut index = start;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        index += 1;
        if trimmed.contains("*/") {
            break;
        }
    }
    index
}

/// Advance past a Go import block (single-line or parenthesised group).
fn consume_go_import(lines: &[String], start: usize) -> usize {
    let trimmed = lines[start].trim();
    if !(trimmed == "import(" || trimmed == "import (" || trimmed.starts_with("import (")) {
        return consume_statement(lines, start);
    }

    let mut index = start + 1;
    while index < lines.len() {
        let line = lines[index].trim();
        index += 1;
        if line.starts_with(')') {
            break;
        }
    }
    index
}

/// Advance past a possibly multi-line statement by tracking bracket depth
/// and line-continuation markers (`\\`, trailing `,`).
fn consume_statement(lines: &[String], start: usize) -> usize {
    let mut index = start;
    let mut paren_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut bracket_depth = 0i32;

    while index < lines.len() {
        let line = lines[index].trim();
        paren_depth += count_char(line, '(') - count_char(line, ')');
        brace_depth += count_char(line, '{') - count_char(line, '}');
        bracket_depth += count_char(line, '[') - count_char(line, ']');

        index += 1;

        let depth_open = paren_depth > 0 || brace_depth > 0 || bracket_depth > 0;
        let explicit_continue = line.ends_with('\\') || line.ends_with(',');
        let explicit_end = line.ends_with(';');

        if !depth_open && (explicit_end || !explicit_continue) {
            break;
        }
    }

    index
}

/// If `start` begins a triple-quoted docstring, advance past it and return
/// the next line index. Returns `None` if no docstring is found.
fn consume_python_docstring(lines: &[String], start: usize) -> Option<usize> {
    if start >= lines.len() {
        return None;
    }

    let trimmed = lines[start].trim_start();
    let Some(delim) = python_docstring_delimiter(trimmed) else {
        return None;
    };

    let Some(prefix_index) = trimmed.find(delim) else {
        return None;
    };
    let rest = &trimmed[prefix_index + delim.len()..];
    if rest.contains(delim) {
        return Some(start + 1);
    }

    let mut index = start + 1;
    while index < lines.len() {
        if lines[index].contains(delim) {
            return Some(index + 1);
        }
        index += 1;
    }

    Some(lines.len())
}

/// Check if `line` starts with a triple-quote delimiter (optionally preceded
/// by string prefix letters like `r`, `b`, `f`, etc.). Returns the delimiter.
fn python_docstring_delimiter(line: &str) -> Option<&'static str> {
    let mut index = 0usize;
    while let Some(ch) = line.as_bytes().get(index) {
        if !matches!(*ch as char, 'r' | 'R' | 'u' | 'U' | 'b' | 'B' | 'f' | 'F') {
            break;
        }
        index += 1;
    }

    let rest = &line[index..];
    if rest.starts_with("\"\"\"") {
        return Some("\"\"\"");
    }
    if rest.starts_with("'''") {
        return Some("'''");
    }

    None
}

fn is_rust_use_statement(line: &str) -> bool {
    line.starts_with("use ") || line.starts_with("pub use ")
}

fn is_rust_mod_declaration(line: &str) -> bool {
    line.starts_with("mod ") || line.starts_with("pub mod ")
}

fn is_rust_extern_crate_statement(line: &str) -> bool {
    line.starts_with("extern crate ") || line.starts_with("pub extern crate ")
}

fn is_ts_js_import_statement(line: &str) -> bool {
    line.starts_with("import ")
}

fn is_ts_js_require_statement(line: &str) -> bool {
    line.starts_with("require(")
        || ((line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var "))
            && (line.contains("= require(") || line.contains("=require(")))
}

fn is_ts_js_export_from_statement(line: &str) -> bool {
    line.starts_with("export ") && line.contains(" from ")
}

fn is_python_import_statement(line: &str) -> bool {
    line.starts_with("import ") || line.starts_with("from ")
}

fn count_char(line: &str, needle: char) -> i32 {
    line.chars().filter(|ch| *ch == needle).count() as i32
}

/// Return the line index where YAML frontmatter ends.
///
/// If the file starts with `---` followed by a closing `---` or `...`, the
/// preamble ends after that delimiter. Otherwise returns `0`.
pub fn markdown_preamble_end(lines: &[String]) -> usize {
    if lines.is_empty() {
        return 0;
    }

    if lines[0].trim() != "---" {
        return 0;
    }

    let mut index = 1;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed == "---" || trimmed == "..." {
            return index + 1;
        }
        index += 1;
    }

    0
}
