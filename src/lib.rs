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
//! - [`fmt`] — Code file index header generation and maintenance.
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`root`] — Project root discovery via `.kdb/`.
//! - [`symbols`] — Multi-language code symbol extraction.
//! - [`tree`] — Filtered tree rendering for project orientation.

pub mod cmd;
pub mod config;
pub mod deps;
pub mod fmt;
pub mod index;
pub mod lsp;
pub mod root;
pub mod symbols;
pub mod tree;
// src/lib.rs
//
