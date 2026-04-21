//! Shared file discovery helpers built on the `ignore` crate.

use anyhow::Result;
use globset::GlobSet;
use ignore::{DirEntry, WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::ignore::path_is_ignored;
use super::paths::normalize_rel_path;

// ------------------------------
// projects/kdb/src/workspace/discover.rs
//
// pub fn discover_files()    L25
// fn rel_path_from_root()    L94
// fn should_visit_entry()    L99
// ------------------------------

/// Discover files under `scope` and return sorted relative paths from `root`.
///
/// Discovery honors `.gitignore` and `.ignore` files through
/// `ignore::WalkBuilder`, plus the compiled `ignore_set` (which already
/// includes `.kdb/ignore` patterns, config patterns, and the `.kdb` dir).
pub fn discover_files(
    root: &Path,
    scope: &Path,
    ignore_set: &GlobSet,
) -> Result<Vec<PathBuf>> {
    let root = root.to_path_buf();
    let ignore_set = ignore_set.clone();

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
            move |entry| should_visit_entry(entry, &root, &ignore_set)
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

    !path_is_ignored(ignore_set, &rel, is_dir)
}
