//! Shared workspace infrastructure.
//!
//! Consolidates root discovery, configuration loading, ignore handling,
//! path normalization, and file discovery into a single module with a
//! [`WorkspaceContext`] struct that owns the canonical workspace state.

pub mod config;
pub mod discover;
pub mod ignore;
pub mod paths;
pub mod root;

use anyhow::{Context, Result};
use globset::GlobSet;
use std::path::{Path, PathBuf};

// ----------------------------------
// projects/kdb/src/workspace/mod.rs
//
// pub mod config                  L7
// pub mod discover                L8
// pub mod ignore                  L9
// pub mod paths                  L10
// pub mod root                   L11
// pub struct WorkspaceContext    L35
//   pub fn discover()            L49
//   pub fn from_root()           L59
// ----------------------------------

/// Shared workspace state: root path, ignore patterns, and compiled matchers.
///
/// Constructed once per CLI command or LSP session and threaded to subsystems
/// that need workspace-level context.
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    /// Canonical absolute path to the workspace root directory.
    pub root: PathBuf,
    /// Raw user-configured ignore patterns from `.kdb/config.toml`.
    pub ignore_patterns: Vec<String>,
    /// Compiled globset built from `ignore_patterns`.
    pub ignore_set: GlobSet,
}

impl WorkspaceContext {
    /// Discover the workspace root from `start` and load configuration.
    ///
    /// Walks upward to find the `.kdb/` marker, loads ignore patterns from
    /// config, and compiles them into a globset.
    pub fn discover(start: &Path) -> Result<Self> {
        let root = root::find_root(start)?;
        Self::from_root(root)
    }

    /// Build context from a known root path.
    ///
    /// Useful when the root is already known (e.g. LSP backend after
    /// initialization).  Merges patterns from `.kdb/ignore` and
    /// `config.toml [index].ignore` into a single compiled globset.
    pub fn from_root(root: PathBuf) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
        let mut ignore_patterns = ignore::load_ignore_file(&root)?;
        let config_patterns = config::load_index_ignores(&root)?;
        ignore_patterns.extend(config_patterns);
        let ignore_set = ignore::build_ignore_globset(&ignore_patterns)?;
        Ok(Self {
            root,
            ignore_patterns,
            ignore_set,
        })
    }
}
