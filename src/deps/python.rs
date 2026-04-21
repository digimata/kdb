use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::index::deps::Dependency;
use crate::workspace::paths::normalize_rel_path;

use super::utils::resolve_file;

// --------------------------------
// projects/kdb/src/deps/python.rs
//
// pub(super) fn collect()      L19
// fn parse_names()             L72
// fn resolve_module()          L96
// fn module_path()            L101
// fn resolve_module_path()    L123
// --------------------------------

pub(super) fn collect(
    root: &Path,
    source_file: &Path,
    source: &str,
    deps: &mut BTreeSet<Dependency>,
) {
    for line in source.lines() {
        let no_comment = line.split('#').next().unwrap_or(line).trim();
        if no_comment.is_empty() {
            continue;
        }

        if let Some(rest) = no_comment.strip_prefix("import ") {
            for module in rest.split(',') {
                let Some(name) = module.split_whitespace().next() else {
                    continue;
                };
                if let Some(file) = resolve_module(root, source_file, name) {
                    deps.insert(Dependency { file, anchor: None });
                }
            }
            continue;
        }

        let Some(rest) = no_comment.strip_prefix("from ") else {
            continue;
        };
        let Some((module, imported)) = rest.split_once(" import ") else {
            continue;
        };

        let module = module.trim();
        if module.chars().any(|ch| ch != '.') {
            if let Some(file) = resolve_module(root, source_file, module) {
                deps.insert(Dependency { file, anchor: None });
            }
        }

        let Some(module_path) = module_path(source_file, module) else {
            continue;
        };
        for name in parse_names(imported) {
            if name == "*" {
                continue;
            }
            let candidate = module_path.join(name.replace('.', "/"));
            if let Some(file) = resolve_module_path(root, &candidate) {
                deps.insert(Dependency { file, anchor: None });
            }
        }
    }
}

fn parse_names(raw: &str) -> Vec<String> {
    let cleaned = raw
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();

    cleaned
        .split(',')
        .filter_map(|item| {
            let name = item.trim();
            if name.is_empty() {
                return None;
            }
            let name = name.split(" as ").next().unwrap_or(name).trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn resolve_module(root: &Path, source_file: &Path, module: &str) -> Option<PathBuf> {
    let module_path = module_path(source_file, module)?;
    resolve_module_path(root, &module_path)
}

fn module_path(source_file: &Path, module: &str) -> Option<PathBuf> {
    let dots = module.chars().take_while(|ch| *ch == '.').count();
    let tail = module[dots..].trim();

    let mut base = if dots == 0 {
        PathBuf::new()
    } else {
        source_file.parent().unwrap_or(Path::new("")).to_path_buf()
    };

    for _ in 1..dots {
        base = base.parent()?.to_path_buf();
    }

    if tail.is_empty() {
        return normalize_rel_path(&base);
    }

    let rel = tail.replace('.', "/");
    normalize_rel_path(&base.join(rel))
}

fn resolve_module_path(root: &Path, module_path: &Path) -> Option<PathBuf> {
    let file_candidate = module_path.with_extension("py");
    if let Some(path) = resolve_file(root, &file_candidate) {
        return Some(path);
    }

    resolve_file(root, &module_path.join("__init__.py"))
}
