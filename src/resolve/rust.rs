use globset::GlobSet;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use toml::Value as TomlValue;
use walkdir::WalkDir;

use super::{
    build_line_starts, line_number_for_offset, normalize_identifier, resolve_file, ImportKind,
    ResolvedImport, RustWorkspaceCache, RustWorkspaceCrate,
};

// ---------------------------------------------
// src/resolve/rust.rs
//
// static MOD_RE                             L54
// static USE_RE                             L59
// struct ParsedManifest                     L64
// struct LocalDependency                    L71
// pub(super) fn build_workspace_cache()     L78
// fn discover_manifest_paths()             L150
// fn parse_manifest()                      L201
// fn parse_crate_root_files()              L240
// fn push_manifest_entry_path()            L264
// fn default_crate_root_files()            L273
// fn normalize_manifest_path()             L281
// fn manifest_src_root()                   L289
// fn collect_dependency_sections()         L297
// fn parse_local_dependency()              L329
// fn resolve_dependency_root()             L383
// fn crate_root_for_name()                 L407
// fn crate_import_name()                   L420
// struct CrateContext                      L425
// pub(super) fn resolve()                  L432
// fn parse_use_prefix()                    L492
// fn resolve_mod_decl()                    L504
// fn resolve_context()                     L529
// fn find_crate_root()                     L563
// fn resolve_use()                         L582
// fn classify_use_kind()                   L661
// fn rust_module_path()                    L683
// fn rust_crate_entry_path()               L700
// fn rust_file_candidates()                L716
// fn looks_like_module_segment()           L724
// fn source_segments()                     L732
// fn imported_names()                      L754
// fn split_brace_group()                   L791
// fn last_segment()                        L803
// fn dedupe_names()                        L819
// ---------------------------------------------

static MOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
        .expect("valid rust mod regex")
});

static USE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([^;]+);").expect("valid rust use regex")
});

#[derive(Debug, Clone, Default)]
struct ParsedManifest {
    package_name: Option<String>,
    crate_root_files: Vec<PathBuf>,
    dependencies: HashMap<String, LocalDependency>,
}

#[derive(Debug, Clone)]
struct LocalDependency {
    alias: String,
    package: Option<String>,
    path: Option<PathBuf>,
    workspace: bool,
}

pub(super) fn build_workspace_cache(root: &Path, ignore_set: &GlobSet) -> RustWorkspaceCache {
    let manifests = discover_manifest_paths(root, ignore_set);
    let mut manifests_by_root = HashMap::new();
    let mut crate_roots_by_name = HashMap::new();

    for rel_manifest in manifests {
        let crate_root = rel_manifest.parent().unwrap_or(Path::new("")).to_path_buf();
        let Some(manifest) = parse_manifest(root, &rel_manifest, &crate_root) else {
            continue;
        };

        if let Some(name) = manifest.package_name.as_ref() {
            crate_roots_by_name
                .entry(name.clone())
                .or_insert_with(|| crate_root.clone());
        }
        manifests_by_root.insert(crate_root, manifest);
    }

    let mut crates_by_root = HashMap::new();
    for (crate_root, manifest) in &manifests_by_root {
        let Some(name) = manifest.package_name.clone() else {
            continue;
        };

        let mut dependency_src_roots = HashMap::new();
        let mut dependency_entry_files = HashMap::new();
        for dependency in manifest.dependencies.values() {
            let Some(alias) = crate_import_name(&dependency.alias) else {
                continue;
            };

            let target_name = dependency.package.as_deref().unwrap_or(&dependency.alias);
            let Some(target_root) = resolve_dependency_root(
                dependency,
                target_name,
                &manifests_by_root,
                &crate_roots_by_name,
            ) else {
                continue;
            };

            if target_root == *crate_root {
                continue;
            }

            let Some(target_manifest) = manifests_by_root.get(&target_root) else {
                continue;
            };

            dependency_src_roots.insert(
                alias.clone(),
                manifest_src_root(&target_root, target_manifest),
            );
            dependency_entry_files.insert(alias, target_manifest.crate_root_files.clone());
        }

        crates_by_root.insert(
            crate_root.clone(),
            RustWorkspaceCrate {
                name,
                src_root: manifest_src_root(crate_root, manifest),
                crate_root_files: manifest.crate_root_files.clone(),
                dependency_src_roots,
                dependency_entry_files,
            },
        );
    }

    RustWorkspaceCache { crates_by_root }
}

fn discover_manifest_paths(root: &Path, ignore_set: &GlobSet) -> Vec<PathBuf> {
    let mut manifests = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            let Some(rel) = super::to_root_relative(root, entry.path()) else {
                return false;
            };
            if rel.as_os_str().is_empty() {
                return true;
            }

            let name = entry.file_name().to_string_lossy();
            if super::IGNORED_DIRS.contains(&name.as_ref()) {
                return false;
            }

            !super::path_is_ignored(ignore_set, &rel, true)
        })
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() != "Cargo.toml" {
            continue;
        }

        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let Some(rel) = super::normalize_rel_path(rel) else {
            continue;
        };
        if super::path_is_ignored(ignore_set, &rel, false) {
            continue;
        }

        manifests.push(rel);
    }

    manifests.sort();
    manifests
}

fn parse_manifest(root: &Path, rel_manifest: &Path, crate_root: &Path) -> Option<ParsedManifest> {
    let raw = fs::read_to_string(root.join(rel_manifest)).ok()?;
    let value = toml::from_str::<TomlValue>(&raw).ok()?;

    let package_name = value
        .get("package")
        .and_then(TomlValue::as_table)
        .and_then(|table| table.get("name"))
        .and_then(TomlValue::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);

    let mut dependencies = HashMap::new();
    let Some(root_table) = value.as_table() else {
        return Some(ParsedManifest {
            package_name,
            crate_root_files: default_crate_root_files(crate_root),
            dependencies,
        });
    };

    let crate_root_files = parse_crate_root_files(root_table, crate_root);

    collect_dependency_sections(root_table, crate_root, &mut dependencies);

    if let Some(targets) = root_table.get("target").and_then(TomlValue::as_table) {
        for target in targets.values().filter_map(TomlValue::as_table) {
            collect_dependency_sections(target, crate_root, &mut dependencies);
        }
    }

    Some(ParsedManifest {
        package_name,
        crate_root_files,
        dependencies,
    })
}

fn parse_crate_root_files(table: &toml::value::Table, crate_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Some(lib) = table.get("lib").and_then(TomlValue::as_table) {
        if let Some(path) = lib.get("path").and_then(TomlValue::as_str) {
            push_manifest_entry_path(&mut files, crate_root, path);
        }
    }

    if let Some(bins) = table.get("bin").and_then(TomlValue::as_array) {
        for bin in bins.iter().filter_map(TomlValue::as_table) {
            if let Some(path) = bin.get("path").and_then(TomlValue::as_str) {
                push_manifest_entry_path(&mut files, crate_root, path);
            }
        }
    }

    if files.is_empty() {
        return default_crate_root_files(crate_root);
    }

    files
}

fn push_manifest_entry_path(files: &mut Vec<PathBuf>, crate_root: &Path, raw: &str) {
    let Some(path) = normalize_manifest_path(crate_root, raw) else {
        return;
    };
    if files.iter().all(|existing| existing != &path) {
        files.push(path);
    }
}

fn default_crate_root_files(crate_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for candidate in ["src/lib.rs", "src/main.rs", "src/mod.rs"] {
        push_manifest_entry_path(&mut files, crate_root, candidate);
    }
    files
}

fn normalize_manifest_path(crate_root: &Path, raw: &str) -> Option<PathBuf> {
    let path = raw.trim();
    if path.is_empty() {
        return None;
    }
    super::normalize_rel_path(&crate_root.join(path))
}

fn manifest_src_root(crate_root: &Path, manifest: &ParsedManifest) -> PathBuf {
    manifest
        .crate_root_files
        .first()
        .and_then(|entry| entry.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| crate_root.join("src"))
}

fn collect_dependency_sections(
    table: &toml::value::Table,
    crate_root: &Path,
    dependencies: &mut HashMap<String, LocalDependency>,
) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(entries) = table.get(section).and_then(TomlValue::as_table) else {
            continue;
        };

        for (alias, value) in entries {
            let Some(dependency) = parse_local_dependency(crate_root, alias, value) else {
                continue;
            };

            let key = dependency.alias.clone();
            match dependencies.get_mut(&key) {
                None => {
                    dependencies.insert(key, dependency);
                }
                Some(existing) => {
                    let replace = existing.path.is_none() && dependency.path.is_some()
                        || (!existing.workspace && dependency.workspace);
                    if replace {
                        *existing = dependency;
                    }
                }
            }
        }
    }
}

fn parse_local_dependency(
    crate_root: &Path,
    alias: &str,
    value: &TomlValue,
) -> Option<LocalDependency> {
    let alias = alias.trim();
    if alias.is_empty() {
        return None;
    }

    let table = value.as_table()?;
    let package = table
        .get("package")
        .and_then(TomlValue::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);
    let workspace = table
        .get("workspace")
        .and_then(TomlValue::as_bool)
        .unwrap_or(false);
    let path = table
        .get("path")
        .and_then(TomlValue::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .and_then(|path| {
            let joined = crate_root.join(path);
            super::normalize_rel_path(&joined)
        })
        .and_then(|path| {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "Cargo.toml")
            {
                super::normalize_rel_path(path.parent().unwrap_or(Path::new("")))
            } else {
                Some(path)
            }
        });

    if path.is_none() && !workspace {
        return None;
    }

    Some(LocalDependency {
        alias: alias.to_string(),
        package,
        path,
        workspace,
    })
}

fn resolve_dependency_root(
    dependency: &LocalDependency,
    target_name: &str,
    manifests_by_root: &HashMap<PathBuf, ParsedManifest>,
    crate_roots_by_name: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = dependency.path.as_ref() {
        if manifests_by_root.contains_key(path) {
            return Some(path.clone());
        }
    }

    if let Some(root) = crate_root_for_name(crate_roots_by_name, target_name) {
        return Some(root.clone());
    }

    if dependency.workspace {
        let fallback = dependency.alias.replace('_', "-");
        return crate_roots_by_name.get(&fallback).cloned();
    }

    None
}

fn crate_root_for_name<'a>(
    crate_roots_by_name: &'a HashMap<String, PathBuf>,
    name: &str,
) -> Option<&'a PathBuf> {
    crate_roots_by_name.get(name).or_else(|| {
        if name.contains('_') {
            crate_roots_by_name.get(&name.replace('_', "-"))
        } else {
            None
        }
    })
}

fn crate_import_name(raw: &str) -> Option<String> {
    normalize_identifier(&raw.replace('-', "_"))
}

#[derive(Debug, Clone)]
struct CrateContext {
    src_root: PathBuf,
    crate_root_files: Vec<PathBuf>,
    dependency_src_roots: HashMap<String, PathBuf>,
    dependency_entry_files: HashMap<String, Vec<PathBuf>>,
}

pub(super) fn resolve(
    root: &Path,
    source_file: &Path,
    source: &str,
    rust_workspace: &RustWorkspaceCache,
) -> Vec<ResolvedImport> {
    let line_starts = build_line_starts(source);
    let crate_context = resolve_context(root, source_file, rust_workspace);
    let mut imports = Vec::new();

    for captures in MOD_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(name) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };

        let resolved_path = resolve_mod_decl(root, source_file, name, &crate_context);
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

fn resolve_mod_decl(
    root: &Path,
    source_file: &Path,
    name: &str,
    context: &CrateContext,
) -> Option<PathBuf> {
    let mut module_dir = source_file.parent().unwrap_or(Path::new("")).to_path_buf();
    if context
        .crate_root_files
        .iter()
        .all(|crate_root_file| crate_root_file != source_file)
    {
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

fn resolve_context(
    root: &Path,
    source_file: &Path,
    rust_workspace: &RustWorkspaceCache,
) -> CrateContext {
    let crate_root = find_crate_root(root, source_file).unwrap_or_default();
    let cached = rust_workspace.crates_by_root.get(&crate_root);
    let src_root = cached
        .map(|crate_info| crate_info.src_root.clone())
        .unwrap_or_else(|| {
            if crate_root.as_os_str().is_empty() {
                PathBuf::from("src")
            } else {
                crate_root.join("src")
            }
        });
    let crate_root_files = cached
        .map(|crate_info| crate_info.crate_root_files.clone())
        .unwrap_or_else(|| default_crate_root_files(&crate_root));
    let dependency_src_roots = cached
        .map(|crate_info| crate_info.dependency_src_roots.clone())
        .unwrap_or_default();
    let dependency_entry_files = cached
        .map(|crate_info| crate_info.dependency_entry_files.clone())
        .unwrap_or_default();

    CrateContext {
        src_root,
        crate_root_files,
        dependency_src_roots,
        dependency_entry_files,
    }
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
    let (mut module, src_root, resolve_crate_entry, entry_files) = match parts.first().copied()? {
        "crate" => {
            cursor += 1;
            (Vec::new(), context.src_root.as_path(), false, Vec::new())
        }
        "self" => {
            cursor += 1;
            (
                source_segments(source_file, &context.src_root)?,
                context.src_root.as_path(),
                false,
                Vec::new(),
            )
        }
        "super" => {
            let mut module = source_segments(source_file, &context.src_root)?;
            while cursor < parts.len() && parts[cursor] == "super" {
                module.pop()?;
                cursor += 1;
            }
            (module, context.src_root.as_path(), false, Vec::new())
        }
        crate_name => {
            let dependency_root = context.dependency_src_roots.get(crate_name)?;
            let entry_files = context
                .dependency_entry_files
                .get(crate_name)
                .cloned()
                .unwrap_or_default();
            cursor += 1;
            (Vec::new(), dependency_root.as_path(), true, entry_files)
        }
    };

    while cursor < parts.len() {
        let part = parts[cursor];
        cursor += 1;
        if part == "self" || part == "*" {
            continue;
        }

        if resolve_crate_entry && !looks_like_module_segment(part) {
            break;
        }
        module.push(part.to_string());
    }

    if module.is_empty() {
        if resolve_crate_entry {
            return rust_crate_entry_path(root, &entry_files, src_root);
        }
        return None;
    }

    rust_module_path(root, src_root, &module).or_else(|| {
        if resolve_crate_entry {
            rust_crate_entry_path(root, &entry_files, src_root)
        } else {
            None
        }
    })
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

fn rust_crate_entry_path(root: &Path, entry_files: &[PathBuf], src_root: &Path) -> Option<PathBuf> {
    for candidate in entry_files {
        if let Some(path) = resolve_file(root, candidate) {
            return Some(path);
        }
    }

    for entry in ["lib.rs", "main.rs", "mod.rs"] {
        let candidate = src_root.join(entry);
        if let Some(path) = resolve_file(root, &candidate) {
            return Some(path);
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

fn looks_like_module_segment(value: &str) -> bool {
    let candidate = value.strip_prefix("r#").unwrap_or(value);
    !candidate.is_empty()
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
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
