//! `kdb` — a compiler and language server for markdown knowledge bases.
//!
//! Treats a directory of markdown files like a codebase: headings are exported
//! symbols, links are imports, and broken references are compile errors.
//!
//! # Modules
//!
//! - [`cmd`] — CLI command implementations (`init`, `check`, `outline`, `symbols`, `fmt`, `lsp`).
//! - [`config`] — Project configuration loading.
//! - [`fmt`] — Code file index header generation and maintenance.
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`root`] — Project root discovery via `.kdb/`.
//! - [`symbols`] — Multi-language code symbol extraction.

pub mod cmd;
pub mod config;
pub mod fmt;
pub mod index;
pub mod lsp;
pub mod root;
pub mod symbols;
// ## Index
//

