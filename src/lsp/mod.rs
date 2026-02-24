//! Language Server Protocol implementation.
//!
//! Provides IDE features for markdown knowledge bases via the LSP:
//!
//! - **Document symbols** — heading outline for the sidebar.
//! - **Go to definition** — jump to the file/heading a link points to.
//! - **Autocomplete** — file and heading completions while typing links.
//! - **Diagnostics** — broken link errors (not yet implemented).
//! - **Hover** — preview linked content (not yet implemented).

mod backend;
mod completion;
mod definition;
mod diagnostics;
mod hover;
mod semantic_tokens;
mod symbols;

pub use backend::serve;
// src/lsp/mod.rs
//

