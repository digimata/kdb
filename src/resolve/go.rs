use std::fs;
use std::path::{Path, PathBuf};

use super::{ImportKind, ResolvedImport, list_go_package_files, normalize_identifier};

// -------------------------------
// src/resolve/go.rs
//
// pub(super) fn resolve()     L18
// fn push_import()            L73
// fn go_module_name()        L112
// fn resolve_import()        L126
// fn parse_import_line()     L155
// fn import_names()          L179
// fn classify_kind()         L195
// -------------------------------

pub(super) fn resolve(root: &Path, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
    let module_name = go_module_name(root);
    let mut imports = Vec::new();
    let mut in_block = false;

    for (index, line) in source.lines().enumerate() {
        let line_no = index + 1;
        let no_comment = line.split("//").next().unwrap_or(line).trim();
        if no_comment.is_empty() {
            continue;
        }

        if in_block {
            if no_comment.starts_with(')') {
                in_block = false;
                continue;
            }

            if let Some((alias, spec)) = parse_import_line(no_comment) {
                push_import(
                    root,
                    source_file,
                    module_name.as_deref(),
                    line_no,
                    alias,
                    &spec,
                    &mut imports,
                );
            }
            continue;
        }

        if no_comment.starts_with("import (") {
            in_block = true;
            continue;
        }

        if let Some(rest) = no_comment.strip_prefix("import ") {
            if let Some((alias, spec)) = parse_import_line(rest) {
                push_import(
                    root,
                    source_file,
                    module_name.as_deref(),
                    line_no,
                    alias,
                    &spec,
                    &mut imports,
                );
            }
        }
    }

    imports
}

fn push_import(
    root: &Path,
    source_file: &Path,
    module_name: Option<&str>,
    line: usize,
    alias: Option<String>,
    spec: &str,
    imports: &mut Vec<ResolvedImport>,
) {
    let resolved_paths = resolve_import(root, source_file, module_name, spec);
    let kind = if !resolved_paths.is_empty() {
        classify_kind(module_name, spec)
    } else {
        ImportKind::External
    };
    let names = import_names(alias.as_deref(), spec);

    if resolved_paths.is_empty() {
        imports.push(ResolvedImport {
            raw: spec.to_string(),
            resolved_path: None,
            kind,
            names,
            line,
        });
        return;
    }

    for resolved_path in resolved_paths {
        imports.push(ResolvedImport {
            raw: spec.to_string(),
            resolved_path: Some(resolved_path),
            kind,
            names: names.clone(),
            line,
        });
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

fn parse_import_line(line: &str) -> Option<(Option<String>, String)> {
    let start = line.find('"')?;
    let end = line[start + 1..].find('"')? + start + 1;
    let spec = line[start + 1..end].trim();
    if spec.is_empty() {
        return None;
    }

    let prefix = line[..start].trim();
    let alias = if prefix.is_empty() {
        None
    } else {
        Some(
            prefix
                .split_whitespace()
                .next()
                .unwrap_or(prefix)
                .to_string(),
        )
    };

    Some((alias, spec.to_string()))
}

fn import_names(alias: Option<&str>, spec: &str) -> Vec<String> {
    if let Some(alias) = alias {
        if alias != "_" && alias != "." {
            return normalize_identifier(alias)
                .map(|name| vec![name])
                .unwrap_or_default();
        }
    }

    spec.rsplit('/')
        .next()
        .and_then(normalize_identifier)
        .map(|name| vec![name])
        .unwrap_or_default()
}

fn classify_kind(module_name: Option<&str>, spec: &str) -> ImportKind {
    if spec.starts_with("./") || spec.starts_with("../") {
        return ImportKind::Relative;
    }

    if module_name.is_some_and(|module| spec == module || spec.starts_with(&format!("{module}/"))) {
        return ImportKind::Workspace;
    }

    ImportKind::External
}
