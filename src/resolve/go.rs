use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    ImportKind, ResolvedImport, list_go_package_files, normalize_identifier, normalize_rel_path,
    to_root_relative,
};

// -----------------------------------
// src/resolve/go.rs
//
// enum GoWorkBlock                L37
// struct ParsedGoWork             L43
// pub struct GoWorkspaceCache     L49
//   pub(super) fn build()         L54
//   pub(super) fn resolve()       L80
// fn push_import()               L124
// fn parse_go_work()             L159
// fn go_module_name()            L220
// fn resolve_import()            L235
// fn workspace_module_match()    L261
// fn parse_import_line()         L288
// fn import_names()              L312
// fn classify_kind()             L328
// fn directive_body()            L343
// fn strip_line_comment()        L356
// fn parse_use_path()            L360
// fn parse_replace_entry()       L365
// fn parse_local_dir()           L381
// fn is_local_path()             L395
// fn trim_go_token()             L406
// fn push_unique_path()          L410
// -----------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GoWorkBlock {
    Use,
    Replace,
}

#[derive(Debug, Default)]
struct ParsedGoWork {
    use_dirs: Vec<PathBuf>,
    replace_dirs: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct GoWorkspaceCache {
    pub modules_by_path: HashMap<String, PathBuf>,
}

impl GoWorkspaceCache {
    pub(super) fn build(root: &Path) -> Self {
        let parsed = parse_go_work(root);
        let mut modules_by_path = HashMap::new();

        for module_dir in &parsed.use_dirs {
            let Some(module_name) = go_module_name(root, module_dir) else {
                continue;
            };
            modules_by_path
                .entry(module_name)
                .or_insert_with(|| module_dir.clone());
        }

        if let Some(module_name) = go_module_name(root, Path::new("")) {
            modules_by_path
                .entry(module_name)
                .or_insert_with(PathBuf::new);
        }

        for (module_path, local_dir) in parsed.replace_dirs {
            modules_by_path.insert(module_path, local_dir);
        }

        Self { modules_by_path }
    }

    pub(super) fn resolve(
        &self,
        root: &Path,
        source_file: &Path,
        source: &str,
    ) -> Vec<ResolvedImport> {
        let mut imports = Vec::new();
        let mut in_block = false;

        for (index, line) in source.lines().enumerate() {
            let line_no = index + 1;
            let no_comment = strip_line_comment(line);
            if no_comment.is_empty() {
                continue;
            }

            if in_block {
                if no_comment.starts_with(')') {
                    in_block = false;
                    continue;
                }

                if let Some((alias, spec)) = parse_import_line(no_comment) {
                    push_import(root, source_file, self, line_no, alias, &spec, &mut imports);
                }
                continue;
            }

            if no_comment.starts_with("import (") {
                in_block = true;
                continue;
            }

            if let Some(rest) = no_comment.strip_prefix("import ") {
                if let Some((alias, spec)) = parse_import_line(rest) {
                    push_import(root, source_file, self, line_no, alias, &spec, &mut imports);
                }
            }
        }

        imports
    }
}

fn push_import(
    root: &Path,
    source_file: &Path,
    go_workspace: &GoWorkspaceCache,
    line: usize,
    alias: Option<String>,
    spec: &str,
    imports: &mut Vec<ResolvedImport>,
) {
    let resolved_paths = resolve_import(root, source_file, go_workspace, spec);
    let kind = classify_kind(spec, !resolved_paths.is_empty());
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

fn parse_go_work(root: &Path) -> ParsedGoWork {
    let source = match fs::read_to_string(root.join("go.work")) {
        Ok(source) => source,
        Err(_) => return ParsedGoWork::default(),
    };

    let mut parsed = ParsedGoWork::default();
    let mut block = None;

    for raw_line in source.lines() {
        let line = strip_line_comment(raw_line);
        if line.is_empty() {
            continue;
        }

        if line == ")" {
            block = None;
            continue;
        }

        match block {
            Some(GoWorkBlock::Use) => {
                if let Some(path) = parse_use_path(root, Path::new(""), line) {
                    push_unique_path(&mut parsed.use_dirs, path);
                }
                continue;
            }
            Some(GoWorkBlock::Replace) => {
                if let Some((module_path, local_dir)) =
                    parse_replace_entry(root, Path::new(""), line)
                {
                    parsed.replace_dirs.insert(module_path, local_dir);
                }
                continue;
            }
            None => {}
        }

        if let Some(body) = directive_body(line, "use") {
            if body == "(" {
                block = Some(GoWorkBlock::Use);
            } else if let Some(path) = parse_use_path(root, Path::new(""), body) {
                push_unique_path(&mut parsed.use_dirs, path);
            }
            continue;
        }

        if let Some(body) = directive_body(line, "replace") {
            if body == "(" {
                block = Some(GoWorkBlock::Replace);
            } else if let Some((module_path, local_dir)) =
                parse_replace_entry(root, Path::new(""), body)
            {
                parsed.replace_dirs.insert(module_path, local_dir);
            }
        }
    }

    parsed
}

fn go_module_name(root: &Path, module_dir: &Path) -> Option<String> {
    let source = fs::read_to_string(root.join(module_dir).join("go.mod")).ok()?;
    for raw_line in source.lines() {
        let line = strip_line_comment(raw_line);
        let Some(body) = directive_body(line, "module") else {
            continue;
        };
        let module_name = trim_go_token(body.split_whitespace().next().unwrap_or(""));
        if !module_name.is_empty() {
            return Some(module_name.to_string());
        }
    }
    None
}

fn resolve_import(
    root: &Path,
    source_file: &Path,
    go_workspace: &GoWorkspaceCache,
    spec: &str,
) -> Vec<PathBuf> {
    if spec.starts_with("./") || spec.starts_with("../") {
        let dir = source_file.parent().unwrap_or(Path::new("")).join(spec);
        return list_go_package_files(root, &dir);
    }

    let Some((module_path, module_dir)) = workspace_module_match(go_workspace, spec) else {
        return Vec::new();
    };

    if spec == module_path {
        return list_go_package_files(root, module_dir);
    }

    let Some(rest) = spec.strip_prefix(module_path) else {
        return Vec::new();
    };

    list_go_package_files(root, &module_dir.join(rest.trim_start_matches('/')))
}

fn workspace_module_match<'a>(
    go_workspace: &'a GoWorkspaceCache,
    spec: &str,
) -> Option<(&'a str, &'a PathBuf)> {
    let mut best_match: Option<(&str, &PathBuf)> = None;

    for (module_path, module_dir) in &go_workspace.modules_by_path {
        let is_match = spec == module_path
            || spec
                .strip_prefix(module_path)
                .is_some_and(|rest| rest.starts_with('/'));
        if !is_match {
            continue;
        }

        let should_replace = match best_match {
            Some((best_path, _)) => module_path.len() > best_path.len(),
            None => true,
        };
        if should_replace {
            best_match = Some((module_path.as_str(), module_dir));
        }
    }

    best_match
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

fn classify_kind(spec: &str, resolved: bool) -> ImportKind {
    if spec.starts_with("./") || spec.starts_with("../") {
        if resolved {
            return ImportKind::Relative;
        }
        return ImportKind::External;
    }

    if resolved {
        return ImportKind::Workspace;
    }

    ImportKind::External
}

fn directive_body<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    let Some(first) = rest.chars().next() else {
        return Some(rest);
    };

    if !first.is_whitespace() && first != '(' {
        return None;
    }

    Some(rest.trim_start())
}

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line).trim()
}

fn parse_use_path(root: &Path, base_dir: &Path, value: &str) -> Option<PathBuf> {
    let token = value.split_whitespace().next()?;
    parse_local_dir(root, base_dir, token)
}

fn parse_replace_entry(root: &Path, base_dir: &Path, value: &str) -> Option<(String, PathBuf)> {
    let (left, right) = value.split_once("=>")?;
    let module_path = trim_go_token(left.split_whitespace().next()?);
    if module_path.is_empty() {
        return None;
    }

    let replacement = trim_go_token(right.split_whitespace().next()?);
    if !is_local_path(replacement) {
        return None;
    }

    let local_dir = parse_local_dir(root, base_dir, replacement)?;
    Some((module_path.to_string(), local_dir))
}

fn parse_local_dir(root: &Path, base_dir: &Path, raw: &str) -> Option<PathBuf> {
    let token = trim_go_token(raw);
    if token.is_empty() {
        return None;
    }

    let path = Path::new(token);
    if path.is_absolute() {
        return to_root_relative(root, path);
    }

    normalize_rel_path(&base_dir.join(path))
}

fn is_local_path(value: &str) -> bool {
    if value == "." || value == ".." {
        return true;
    }
    if value.starts_with("./") || value.starts_with("../") {
        return true;
    }

    Path::new(value).is_absolute()
}

fn trim_go_token(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('`')
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if paths.iter().all(|existing| existing != &candidate) {
        paths.push(candidate);
    }
}
