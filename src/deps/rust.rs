use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::index::deps::Dependency;

use super::utils::resolve_file;

// --------------------------
// src/deps/rust.rs
//
// fn collect()              L24
// fn parse_use_prefix()     L48
// fn resolve_mod_decl()     L60
// fn resolve_use()          L81
// fn rust_module_path()    L122
// fn rust_file_candidates() L139
// fn source_segments()      L147
// --------------------------

static MOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
        .expect("valid rust mod regex")
});

static USE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([^;]+);").expect("valid rust use regex")
});

pub(super) fn collect(
    root: &Path,
    source_file: &Path,
    source: &str,
    deps: &mut BTreeSet<Dependency>,
) {
    for captures in MOD_RE.captures_iter(source) {
        let Some(name) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if let Some(file) = resolve_mod_decl(root, source_file, name) {
            deps.insert(Dependency { file, anchor: None });
        }
    }

    for captures in USE_RE.captures_iter(source) {
        let Some(path) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(prefix) = parse_use_prefix(path) else {
            continue;
        };
        if let Some(file) = resolve_use(root, source_file, &prefix) {
            deps.insert(Dependency { file, anchor: None });
        }
    }
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

fn resolve_use(root: &Path, source_file: &Path, path: &str) -> Option<PathBuf> {
    let parts = path
        .split("::")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    let current = source_segments(source_file)?;
    let mut cursor = 0usize;
    let mut module = match parts.first().copied()? {
        "crate" => {
            cursor += 1;
            Vec::new()
        }
        "self" => {
            cursor += 1;
            current
        }
        "super" => {
            let mut module = current;
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

    rust_module_path(root, &module)
}

fn rust_module_path(root: &Path, module: &[String]) -> Option<PathBuf> {
    if module.is_empty() {
        return None;
    }

    for size in (1..=module.len()).rev() {
        let prefix = &module[..size];
        for candidate in rust_file_candidates(prefix) {
            if let Some(path) = resolve_file(root, &candidate) {
                return Some(path);
            }
        }
    }

    None
}

fn rust_file_candidates(module: &[String]) -> [PathBuf; 2] {
    let mut base = PathBuf::from("src");
    for part in module {
        base.push(part);
    }
    [base.with_extension("rs"), base.join("mod.rs")]
}

fn source_segments(source_file: &Path) -> Option<Vec<String>> {
    let rel = source_file.strip_prefix("src").ok()?;
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
