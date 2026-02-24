use std::path::{Path, PathBuf};

use super::{normalize_identifier, normalize_rel_path, resolve_file, ImportKind, ResolvedImport};

// --------------------------------
// src/resolve/python.rs
//
// pub(super) fn resolve()      L18
// fn split_alias()            L100
// fn resolve_module()         L106
// fn module_path()            L111
// fn resolve_module_path()    L133
// fn parse_names()            L142
// fn module_binding_name()    L173
// fn classify_kind()          L183
// --------------------------------

pub(super) fn resolve(root: &Path, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
    let mut imports = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_no = index + 1;
        let no_comment = line.split('#').next().unwrap_or(line).trim();
        if no_comment.is_empty() {
            continue;
        }

        if let Some(rest) = no_comment.strip_prefix("import ") {
            for module in rest.split(',') {
                let item = module.trim();
                if item.is_empty() {
                    continue;
                }

                let (module_name, alias) = split_alias(item);
                let resolved_path = resolve_module(root, source_file, module_name);
                let kind = classify_kind(module_name, resolved_path.is_some());
                let names = module_binding_name(module_name, alias)
                    .map(|name| vec![name])
                    .unwrap_or_default();

                imports.push(ResolvedImport {
                    raw: module_name.to_string(),
                    resolved_path,
                    kind,
                    names,
                    line: line_no,
                });
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
            let resolved_path = resolve_module(root, source_file, module);
            let kind = classify_kind(module, resolved_path.is_some());
            imports.push(ResolvedImport {
                raw: module.to_string(),
                resolved_path,
                kind,
                names: Vec::new(),
                line: line_no,
            });
        }

        let Some(module_path) = module_path(source_file, module) else {
            continue;
        };
        for (name, local_name) in parse_names(imported) {
            if name == "*" {
                continue;
            }
            let candidate = module_path.join(name.replace('.', "/"));
            let resolved_path = resolve_module_path(root, &candidate);
            let kind = classify_kind(module, resolved_path.is_some());
            let names = normalize_identifier(&local_name)
                .map(|value| vec![value])
                .unwrap_or_default();

            imports.push(ResolvedImport {
                raw: format!("{module}.{name}"),
                resolved_path,
                kind,
                names,
                line: line_no,
            });
        }
    }

    imports
}

fn split_alias(item: &str) -> (&str, Option<&str>) {
    item.split_once(" as ")
        .map(|(name, alias)| (name.trim(), Some(alias.trim())))
        .unwrap_or((item.trim(), None))
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

fn parse_names(raw: &str) -> Vec<(String, String)> {
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

            let (imported, local) = split_alias(name);
            let imported = imported.trim();
            if imported.is_empty() {
                return None;
            }

            let local = local
                .and_then(normalize_identifier)
                .or_else(|| imported.rsplit('.').next().and_then(normalize_identifier))
                .unwrap_or_else(|| imported.to_string());

            Some((imported.to_string(), local))
        })
        .collect()
}

fn module_binding_name(module: &str, alias: Option<&str>) -> Option<String> {
    if let Some(alias) = alias {
        return normalize_identifier(alias);
    }

    let trimmed = module.trim_start_matches('.');
    let local = trimmed.split('.').next().unwrap_or(trimmed);
    normalize_identifier(local)
}

fn classify_kind(module: &str, has_resolved_path: bool) -> ImportKind {
    if !has_resolved_path {
        return ImportKind::External;
    }

    if module.starts_with('.') {
        ImportKind::Relative
    } else {
        ImportKind::Workspace
    }
}
