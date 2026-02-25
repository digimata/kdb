//! Canonical ignore handling for file discovery.
//!
//! Provides the single source of truth for always-ignored directories and
//! user-configured ignore pattern compilation and matching.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::Path;

// ------------------------------------
// src/project/ignore.rs
//
// pub const ALWAYS_IGNORED_DIRS    L22
// pub fn build_ignore_globset()    L37
// pub fn path_is_ignored()         L53
// ------------------------------------

/// Directories that are always excluded from discovery and tree output.
///
/// These are common build artifacts, version control, and dependency
/// directories that never contain useful knowledge-base content.
pub const ALWAYS_IGNORED_DIRS: &[&str] = &[
    ".kdb",
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".next",
    ".cache",
    "vendor",
    "__pycache__",
    ".venv",
];

/// Compile user-supplied ignore glob patterns into a `GlobSet`.
pub fn build_ignore_globset(ignore_patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in ignore_patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("invalid ignore pattern `{pattern}`"))?;
        builder.add(glob);
    }

    builder.build().context("failed to compile ignore patterns")
}

/// Check whether `rel_path` matches any pattern in the ignore set.
///
/// For directories, this also tests the path with a trailing slash.
pub fn path_is_ignored(ignore_set: &GlobSet, rel_path: &Path, is_dir: bool) -> bool {
    let slash = rel_path.to_string_lossy().replace('\\', "/");
    if slash.is_empty() {
        return false;
    }

    if ignore_set.is_match(&slash) {
        return true;
    }

    if is_dir {
        return ignore_set.is_match(format!("{slash}/"));
    }

    false
}
