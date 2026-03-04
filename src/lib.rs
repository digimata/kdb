//! `kdb` — a compiler and language server for markdown knowledge bases.
//!
//! Treats a directory of markdown files like a codebase: headings are exported
//! symbols, links are imports, and broken references are compile errors.
//!
//! # Modules
//!
//! - [`cmd`] — CLI command implementations (`init`, `check`, `render`, `tree`, `symbols`, `fmt`, `lsp`).
//! - [`deps`] — Code dependency extraction for `kdb deps`.
//! - [`fmt`] — Code file index header generation and maintenance.
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lang`] — Shared code language identifiers and file-type detection.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`project`] — Shared project infrastructure (root, config, discovery, paths, ignore).
//! - [`render`] — Transclusion resolution for `![[file#heading]]` embeds.
//! - [`resolve`] — Workspace-aware code import resolution.
//! - [`symbols`] — Multi-language code symbol extraction.
//! - [`tree`] — Filtered tree rendering for project orientation.

pub mod cmd;
pub mod deps;
pub mod fmt;
pub mod index;
pub mod lang;
pub mod lsp;
pub mod project;
pub mod render;
pub mod resolve;
pub mod symbols;
pub mod tree;
pub mod update;

// ----------------------
// kdb/src/lib.rs
//
// pub mod cmd        L20
// pub mod deps       L21
// pub mod fmt        L22
// pub mod index      L23
// pub mod lang       L24
// pub mod lsp        L25
// pub mod project    L26
// pub mod render     L27
// pub mod resolve    L28
// pub mod symbols    L29
// pub mod tree       L30
// pub mod update     L31
// ----------------------

