//! `kdb` — a compiler and language server for markdown knowledge bases.
//!
//! Treats a directory of markdown files like a codebase: headings are exported
//! symbols, links are imports, and broken references are compile errors.
//!
//! # Modules
//!
//! - [`cmd`] — CLI command implementations (`init`, `check`, `outline`, `tree`, `symbols`, `fmt`, `lsp`).
//! - [`config`] — Project configuration loading.
//! - [`deps`] — Code dependency extraction for `kdb deps`.
//! - [`discovery`] — Shared file discovery and ignore handling.
//! - [`fmt`] — Code file index header generation and maintenance.
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lang`] — Shared code language identifiers and file-type detection.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`resolve`] — Workspace-aware code import resolution.
//! - [`root`] — Project root discovery via `.kdb/`.
//! - [`symbols`] — Multi-language code symbol extraction.
//! - [`tree`] — Filtered tree rendering for project orientation.

pub mod cmd;
pub mod config;
pub mod deps;
pub mod discovery;
pub mod fmt;
pub mod index;
pub mod lang;
pub mod lsp;
pub mod resolve;
pub mod root;
pub mod symbols;
pub mod tree;

// ------------------------
// src/lib.rs
//
// pub mod cmd          L21
// pub mod config       L22
// pub mod deps         L23
// pub mod discovery    L24
// pub mod fmt          L25
// pub mod index        L26
// pub mod lang         L27
// pub mod lsp          L28
// pub mod resolve      L29
// pub mod root         L30
// pub mod symbols      L31
// pub mod tree         L32
// ------------------------
