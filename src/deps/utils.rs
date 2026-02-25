use std::fs;
use std::path::{Path, PathBuf};

use crate::project::paths::normalize_rel_path;

// --------------------------------------------
// src/deps/utils.rs
//
// pub(super) fn resolve_with_exts()        L14
// pub(super) fn resolve_file()             L40
// pub(super) fn list_go_package_files()    L49
// --------------------------------------------

pub(super) fn resolve_with_exts(root: &Path, base: &Path, exts: &[&str]) -> Option<PathBuf> {
    if base.extension().is_some() {
        return resolve_file(root, base);
    }

    if let Some(path) = resolve_file(root, base) {
        return Some(path);
    }

    for ext in exts {
        let candidate = base.with_extension(ext);
        if let Some(path) = resolve_file(root, &candidate) {
            return Some(path);
        }
    }

    for ext in exts {
        let candidate = base.join(format!("index.{ext}"));
        if let Some(path) = resolve_file(root, &candidate) {
            return Some(path);
        }
    }

    None
}

pub(super) fn resolve_file(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let rel = normalize_rel_path(candidate)?;
    if root.join(&rel).is_file() {
        Some(rel)
    } else {
        None
    }
}

pub(super) fn list_go_package_files(root: &Path, dir: &Path) -> Vec<PathBuf> {
    let Some(rel_dir) = normalize_rel_path(dir) else {
        return Vec::new();
    };

    let abs_dir = root.join(&rel_dir);
    let Ok(entries) = fs::read_dir(&abs_dir) else {
        return Vec::new();
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_go = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("go"));
        if !is_go || !path.is_file() {
            continue;
        }

        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let Some(rel) = normalize_rel_path(rel) else {
            continue;
        };
        files.push(rel);
    }

    files.sort();
    files
}
