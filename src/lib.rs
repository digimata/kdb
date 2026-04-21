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
//! - [`projects`] — Projects table access (slug, name, path, status).
//! - [`render`] — Transclusion resolution for `![[file#heading]]` embeds.
//! - [`resolve`] — Workspace-aware code import resolution.
//! - [`symbols`] — Multi-language code symbol extraction.
//! - [`tasks`] — Tasks table access (per-project seq, statuses, priorities).
//! - [`tree`] — Filtered tree rendering for workspace orientation.
//! - [`workspace`] — Shared workspace infrastructure (root, config, discovery, paths, ignore).

pub mod cmd;
pub mod cycles;
pub mod db;
pub mod deps;
pub mod fmt;
pub mod index;
pub mod labels;
pub mod lang;
pub mod lsp;
pub mod materialize;
pub mod projects;
pub mod render;
pub mod resolve;
pub mod symbols;
pub mod tasks;
pub mod tasks_import;
pub mod tree;
pub mod update;
pub mod workspace;

// ---------------------------
// projects/kdb/src/lib.rs
//
// pub mod cmd             L24
// pub mod cycles          L25
// pub mod db              L26
// pub mod deps            L27
// pub mod fmt             L28
// pub mod index           L29
// pub mod labels          L30
// pub mod lang            L31
// pub mod lsp             L32
// pub mod materialize     L33
// pub mod projects        L34
// pub mod render          L35
// pub mod resolve         L36
// pub mod symbols         L37
// pub mod tasks           L38
// pub mod tasks_import    L39
// pub mod tree            L40
// pub mod update          L41
// pub mod workspace       L42
// ---------------------------

