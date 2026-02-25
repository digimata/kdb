//! Canonical path normalization utilities.
//!
//! These helpers resolve `.` and `..` in relative paths without touching the
//! filesystem, and are used throughout kdb wherever vault-relative paths are
//! constructed or validated.

use std::path::{Component, Path, PathBuf};

// ----------------------------------
// src/project/paths.rs
//
// pub fn normalize_rel_path()    L19
// ----------------------------------

/// Normalize a relative path by resolving `.` and `..` components.
///
/// Returns `None` if the path would escape above the root (i.e. more `..`
/// components than depth), or if it contains absolute path components.
pub fn normalize_rel_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(normalized)
}
