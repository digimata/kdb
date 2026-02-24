//! Shared file discovery helpers built on the `ignore` crate.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::index::normalize_rel_path;

// -------------------------------------
// src/discovery.rs
//
// pub fn build_ignore_globset()     L22
// pub fn path_is_ignored()          L38
// pub fn discover_files()           L60
// fn rel_path_from_root()          L134
// fn should_visit_entry()          L139
// -------------------------------------

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

/// Discover files under `scope` and return sorted relative paths from `root`.
///
/// Discovery honors `.gitignore` and `.ignore` files through
/// `ignore::WalkBuilder`, plus user-defined `ignore_set` patterns and
/// caller-specified always-ignored directory names.
pub fn discover_files(
    root: &Path,
    scope: &Path,
    ignore_set: &GlobSet,
    ignored_dirs: &[&str],
) -> Result<Vec<PathBuf>> {
    let root = root.to_path_buf();
    let ignore_set = ignore_set.clone();
    let ignored_dirs = ignored_dirs
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();

    let mut walker = WalkBuilder::new(scope);
    walker
        .follow_links(false)
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(true)
        .require_git(false)
        .filter_entry({
            let root = root.clone();
            let ignore_set = ignore_set.clone();
            move |entry| should_visit_entry(entry, &root, &ignore_set, &ignored_dirs)
        });

    let collected = Arc::new(Mutex::new(Vec::new()));
    let walker = walker.build_parallel();

    walker.run(|| {
        let root = root.clone();
        let ignore_set = ignore_set.clone();
        let collected = Arc::clone(&collected);

        Box::new(move |result| {
            let Ok(entry) = result else {
                return WalkState::Continue;
            };

            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                return WalkState::Continue;
            }

            let Some(rel) = rel_path_from_root(&root, entry.path()) else {
                return WalkState::Continue;
            };

            if path_is_ignored(&ignore_set, &rel, false) {
                return WalkState::Continue;
            }

            let mut paths = collected
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            paths.push(rel);
            WalkState::Continue
        })
    });

    let mut paths = collected
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn rel_path_from_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}

fn should_visit_entry(
    entry: &DirEntry,
    root: &Path,
    ignore_set: &GlobSet,
    ignored_dirs: &[String],
) -> bool {
    let is_dir = entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir());
    let Some(rel) = rel_path_from_root(root, entry.path()) else {
        return false;
    };

    if rel.as_os_str().is_empty() {
        return true;
    }

    if is_dir {
        let name = entry.file_name().to_string_lossy();
        if ignored_dirs.iter().any(|ignored| ignored == name.as_ref()) {
            return false;
        }
    }

    !path_is_ignored(ignore_set, &rel, is_dir)
}
