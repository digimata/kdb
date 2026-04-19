//! `kdb` — a compiler and language server for markdown knowledge bases.
//!
//! Treats a directory of markdown files like a codebase: headings are exported
//! symbols, links are imports, and broken references are compile errors.
//!
//! # Modules
//!
//! - [`cmd`] — CLI command implementations (`init`, `check`, `render`, `tree`, `symbols`, `fmt`, `lsp`).
//! - [`db`] — SQLite-backed relational layer (projects, cycles, tasks, labels).
//! - [`deps`] — Code dependency extraction for `kdb deps`.
//! - [`fmt`] — Code file index header generation and maintenance.
//! - [`index`] — Markdown parser, vault indexer, and link resolver.
//! - [`lang`] — Shared code language identifiers and file-type detection.
//! - [`lsp`] — Language Server Protocol implementation.
//! - [`materialize`] — DB → markdown materialization for per-project TODO files.
//! - [`project`] — Shared project infrastructure (root, config, discovery, paths, ignore).
//! - [`projects`] — Projects table access (slug, name, path, status).
//! - [`render`] — Transclusion resolution for `![[file#heading]]` embeds.
//! - [`resolve`] — Workspace-aware code import resolution.
//! - [`symbols`] — Multi-language code symbol extraction.
//! - [`tasks`] — Tasks table access (per-project seq, statuses, priorities).
//! - [`tree`] — Filtered tree rendering for project orientation.

pub mod cmd;
pub mod db;
pub mod deps;
pub mod fmt;
pub mod index;
pub mod lang;
pub mod lsp;
pub mod materialize;
pub mod project;
pub mod projects;
pub mod render;
pub mod resolve;
pub mod symbols;
pub mod tasks;
pub mod tree;
pub mod update;

// ----------------------
// projects/kdb/src/lib.rs
//
// pub mod cmd        L21
// pub mod db         L22
// pub mod deps       L23
// pub mod fmt        L24
// pub mod index      L25
// pub mod lang         L26
// pub mod lsp          L27
// pub mod materialize  L28
// pub mod project      L29
// pub mod projects     L30
// pub mod render       L31
// pub mod resolve      L32
// pub mod symbols      L33
// pub mod tasks        L34
// pub mod tree         L35
// pub mod update       L36
// ----------------------

