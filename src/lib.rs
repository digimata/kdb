//! `kdb` — a compiler and language server for markdown knowledge bases.
//!
//! Treats a directory of markdown files like a codebase: headings are exported
//! symbols, links are imports, and broken references are compile errors.
//!
//! # Modules
//!
//! - [`cmd`] — CLI command implementations (`check`, `outline`, `lsp`).
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`root`] — Project root discovery via `.kdb/`.

pub mod cmd;
pub mod index;
pub mod lsp;
pub mod root;
