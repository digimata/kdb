use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::index::deps::Dependency;

use super::utils::list_go_package_files;

// -------------------------
// src/deps/go.rs
//
// fn collect()             L15
// fn go_module_name()      L52
// fn resolve_import()       L67
// fn quoted_value()         L96
// -------------------------

pub(super) fn collect(
    root: &Path,
    source_file: &Path,
    source: &str,
    deps: &mut BTreeSet<Dependency>,
) {
    let module_name = go_module_name(root);
    let mut in_block = false;

    for line in source.lines() {
        let no_comment = line.split("//").next().unwrap_or(line).trim();
        if no_comment.is_empty() {
            continue;
        }

        if in_block {
            if no_comment.starts_with(')') {
                in_block = false;
                continue;
            }

            if let Some(spec) = quoted_value(no_comment) {
                for file in resolve_import(root, source_file, module_name.as_deref(), &spec) {
                    deps.insert(Dependency { file, anchor: None });
                }
            }
            continue;
        }

        if no_comment.starts_with("import (") {
            in_block = true;
            continue;
        }

        if no_comment.starts_with("import ") {
            if let Some(spec) = quoted_value(no_comment) {
                for file in resolve_import(root, source_file, module_name.as_deref(), &spec) {
                    deps.insert(Dependency { file, anchor: None });
                }
            }
        }
    }
}

fn go_module_name(root: &Path) -> Option<String> {
    let source = fs::read_to_string(root.join("go.mod")).ok()?;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("module ") {
            let module_name = value.trim();
            if !module_name.is_empty() {
                return Some(module_name.to_string());
            }
        }
    }
    None
}

fn resolve_import(
    root: &Path,
    source_file: &Path,
    module_name: Option<&str>,
    spec: &str,
) -> Vec<PathBuf> {
    if spec.starts_with("./") || spec.starts_with("../") {
        let dir = source_file.parent().unwrap_or(Path::new("")).join(spec);
        return list_go_package_files(root, &dir);
    }

    let Some(module_name) = module_name else {
        return Vec::new();
    };

    if spec == module_name {
        return list_go_package_files(root, Path::new("."));
    }

    let Some(rest) = spec.strip_prefix(module_name) else {
        return Vec::new();
    };
    if !rest.starts_with('/') {
        return Vec::new();
    }

    list_go_package_files(root, Path::new(rest.trim_start_matches('/')))
}

fn quoted_value(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let end = line[start + 1..].find('"')? + start + 1;
    Some(line[start + 1..end].to_string())
}
