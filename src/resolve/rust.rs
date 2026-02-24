use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::{
    ImportKind, ResolvedImport, build_line_starts, line_number_for_offset, normalize_identifier,
    resolve_file,
};

// ---------------------------------
// src/resolve/rust.rs
//
// static MOD_RE                 L29
// static USE_RE                 L34
// pub(super) fn resolve()       L38
// fn parse_use_prefix()         L92
// fn resolve_mod_decl()        L104
// fn resolve_use()             L125
// fn classify_use_kind()       L169
// fn rust_module_path()        L191
// fn rust_file_candidates()    L208
// fn source_segments()         L216
// fn imported_names()          L238
// fn split_brace_group()       L275
// fn last_segment()            L287
// fn dedupe_names()            L303
// ---------------------------------

static MOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
        .expect("valid rust mod regex")
});

static USE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([^;]+);").expect("valid rust use regex")
});

#[derive(Debug, Clone)]
struct CrateContext {
    src_root: PathBuf,
}

pub(super) fn resolve(root: &Path, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
    let line_starts = build_line_starts(source);
    let crate_context = resolve_context(root, source_file);
    let mut imports = Vec::new();

    for captures in MOD_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(name) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };

        let resolved_path = resolve_mod_decl(root, source_file, name);
        let kind = if resolved_path.is_some() {
            ImportKind::Relative
        } else {
            ImportKind::External
        };

        imports.push(ResolvedImport {
            raw: format!("mod {name}"),
            resolved_path,
            kind,
            names: vec![name.to_string()],
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    for captures in USE_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(path) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };

        let prefix = parse_use_prefix(path);
        let resolved_path = prefix
            .as_deref()
            .and_then(|value| resolve_use(root, source_file, value, &crate_context));
        let kind = classify_use_kind(prefix.as_deref(), resolved_path.is_some());

        imports.push(ResolvedImport {
            raw: path.to_string(),
            resolved_path,
            kind,
            names: imported_names(path),
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    imports
}

fn parse_use_prefix(path: &str) -> Option<String> {
    let head = path.split('{').next().unwrap_or(path).trim();
    let head = head.split(',').next().unwrap_or(head).trim();
    let head = head.split(" as ").next().unwrap_or(head).trim();
    let head = head.trim_end_matches(':').trim_end_matches(':').trim();
    if head.is_empty() {
        None
    } else {
        Some(head.to_string())
    }
}

fn resolve_mod_decl(root: &Path, source_file: &Path, name: &str) -> Option<PathBuf> {
    let mut module_dir = source_file.parent().unwrap_or(Path::new("")).to_path_buf();
    let file_name = source_file
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if !matches!(file_name, "mod.rs" | "lib.rs" | "main.rs") {
        let stem = source_file.file_stem()?.to_str()?;
        module_dir.push(stem);
    }

    let base = module_dir.join(name);
    let file_candidate = base.with_extension("rs");
    if let Some(path) = resolve_file(root, &file_candidate) {
        return Some(path);
    }

    resolve_file(root, &base.join("mod.rs"))
}

fn resolve_context(root: &Path, source_file: &Path) -> CrateContext {
    let crate_root = find_crate_root(root, source_file).unwrap_or_default();
    let src_root = if crate_root.as_os_str().is_empty() {
        PathBuf::from("src")
    } else {
        crate_root.join("src")
    };

    CrateContext { src_root }
}

fn find_crate_root(root: &Path, source_file: &Path) -> Option<PathBuf> {
    let mut dir = source_file.parent().unwrap_or(Path::new("")).to_path_buf();

    loop {
        let manifest_rel = if dir.as_os_str().is_empty() {
            PathBuf::from("Cargo.toml")
        } else {
            dir.join("Cargo.toml")
        };
        if root.join(&manifest_rel).is_file() {
            return Some(dir);
        }

        if !dir.pop() {
            return None;
        }
    }
}

fn resolve_use(
    root: &Path,
    source_file: &Path,
    path: &str,
    context: &CrateContext,
) -> Option<PathBuf> {
    let parts = path
        .split("::")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    let mut cursor = 0usize;
    let mut module = match parts.first().copied()? {
        "crate" => {
            cursor += 1;
            Vec::new()
        }
        "self" => {
            cursor += 1;
            source_segments(source_file, &context.src_root)?
        }
        "super" => {
            let mut module = source_segments(source_file, &context.src_root)?;
            while cursor < parts.len() && parts[cursor] == "super" {
                module.pop()?;
                cursor += 1;
            }
            module
        }
        _ => return None,
    };

    while cursor < parts.len() {
        let part = parts[cursor];
        cursor += 1;
        if part == "self" || part == "*" {
            continue;
        }
        module.push(part.to_string());
    }

    rust_module_path(root, &context.src_root, &module)
}

fn classify_use_kind(prefix: Option<&str>, has_resolved_path: bool) -> ImportKind {
    if !has_resolved_path {
        return ImportKind::External;
    }

    let Some(prefix) = prefix else {
        return ImportKind::External;
    };
    if prefix == "crate" || prefix.starts_with("crate::") {
        return ImportKind::Workspace;
    }
    if prefix == "self"
        || prefix.starts_with("self::")
        || prefix == "super"
        || prefix.starts_with("super::")
    {
        return ImportKind::Relative;
    }

    ImportKind::Workspace
}

fn rust_module_path(root: &Path, src_root: &Path, module: &[String]) -> Option<PathBuf> {
    if module.is_empty() {
        return None;
    }

    for size in (1..=module.len()).rev() {
        let prefix = &module[..size];
        for candidate in rust_file_candidates(src_root, prefix) {
            if let Some(path) = resolve_file(root, &candidate) {
                return Some(path);
            }
        }
    }

    None
}

fn rust_file_candidates(src_root: &Path, module: &[String]) -> [PathBuf; 2] {
    let mut base = src_root.to_path_buf();
    for part in module {
        base.push(part);
    }
    [base.with_extension("rs"), base.join("mod.rs")]
}

fn source_segments(source_file: &Path, src_root: &Path) -> Option<Vec<String>> {
    let rel = source_file.strip_prefix(src_root).ok()?;
    let file_name = rel.file_name()?.to_str()?;

    let mut segments = rel
        .parent()
        .into_iter()
        .flat_map(|path| path.iter())
        .filter_map(|part| part.to_str().map(ToString::to_string))
        .collect::<Vec<_>>();

    match file_name {
        "lib.rs" | "main.rs" | "mod.rs" => {}
        _ => {
            let stem = rel.file_stem()?.to_str()?;
            segments.push(stem.to_string());
        }
    }

    Some(segments)
}

fn imported_names(path: &str) -> Vec<String> {
    let trimmed = path.trim();
    if let Some((prefix, body)) = split_brace_group(trimmed) {
        let mut names = Vec::new();
        for item in body.split(',') {
            let token = item.trim();
            if token.is_empty() || token == "*" {
                continue;
            }

            if token == "self" {
                if let Some(name) = last_segment(prefix) {
                    names.push(name);
                }
                continue;
            }

            let local = token
                .split_once(" as ")
                .map(|(_, alias)| alias)
                .unwrap_or(token);
            if let Some(name) = last_segment(local) {
                names.push(name);
            }
        }
        return dedupe_names(names);
    }

    let local = trimmed
        .split_once(" as ")
        .map(|(_, alias)| alias)
        .unwrap_or(trimmed);
    last_segment(local)
        .map(|name| vec![name])
        .unwrap_or_default()
}

fn split_brace_group(input: &str) -> Option<(&str, &str)> {
    let start = input.find('{')?;
    let end = input.rfind('}')?;
    if end <= start {
        return None;
    }
    Some((
        input[..start].trim_end_matches(':').trim(),
        &input[start + 1..end],
    ))
}

fn last_segment(raw: &str) -> Option<String> {
    let value = raw
        .split("::")
        .last()
        .unwrap_or(raw)
        .trim()
        .trim_matches('{')
        .trim_matches('}')
        .trim();
    if matches!(value, "" | "self" | "super" | "crate" | "*") {
        return None;
    }

    normalize_identifier(value)
}

fn dedupe_names(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for name in names {
        if seen.insert(name.clone()) {
            deduped.push(name);
        }
    }
    deduped
}
