//! `kdb` — a compiler and language server for markdown knowledge bases.
//!
//! Treats a directory of markdown files like a codebase: headings are exported
//! symbols, links are imports, and broken references are compile errors.
//!
//! # Modules
//!
//! - [`cmd`] — CLI command implementations (`init`, `check`, `outline`, `tree`, `symbols`, `fmt`, `lsp`).
//! - [`deps`] — Code dependency extraction for `kdb deps`.
//! - [`fmt`] — Code file index header generation and maintenance.
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lang`] — Shared code language identifiers and file-type detection.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`project`] — Shared project infrastructure (root, config, discovery, paths, ignore).
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
// pub mod cmd        L19
// pub mod deps       L20
// pub mod fmt        L21
// pub mod index      L22
// pub mod lang       L23
// pub mod lsp        L24
// pub mod project    L25
// pub mod resolve    L26
// pub mod symbols    L27
// pub mod tree       L28
// pub mod update     L29
// ----------------------

