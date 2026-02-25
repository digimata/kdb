//! Workspace-aware code import resolution.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

use crate::lang::CodeLanguage;

mod go;
mod python;
mod rust;
mod tsjs;

// ---------------------------------------------------
// src/resolve/mod.rs
//
// mod go                                          L14
// mod python                                      L15
// mod rust                                        L16
// mod tsjs                                        L17
// pub type WorkspacePackages                      L74
// pub struct GoWorkspaceCache                     L77
// pub struct PythonWorkspaceCache                 L82
// pub struct RustWorkspaceCrate                   L88
// pub struct RustWorkspaceCache                   L97
// pub enum ImportKind                            L102
// pub struct ResolvedImport                      L110
// pub struct WorkspaceImportIndex                L119
// pub fn build_workspace_import_index()          L127
// pub fn resolve_imports_for_language()          L181
// const IGNORED_DIRS                             L201
// const TSJS_EXTS                                L215
// fn build_ignore_globset()                      L219
// fn discover_code_files()                       L232
// fn discover_workspace_packages()               L287
// struct WorkspacePatternSet                     L362
// fn workspace_patterns()                        L367
// fn read_pnpm_workspace_patterns()              L391
// fn read_package_json_workspace_patterns()      L437
// fn compile_globset()                           L474
// fn workspace_path_allowed()                    L490
// fn globset_matches_path()                      L508
// fn package_name_from_manifest()                L520
// pub(super) fn classify_local_kind()            L532
// pub(super) fn sanitize_specifier()             L547
// pub(super) fn normalize_identifier()           L564
// pub(super) fn resolve_with_exts()              L592
// pub(super) fn resolve_file()                   L618
// fn canonicalize_existing_rel_path()            L627
// pub(super) fn list_go_package_files()          L665
// pub(super) fn build_line_starts()              L699
// pub(super) fn line_number_for_offset()         L709
// pub(super) fn normalize_rel_path()             L718
// pub(super) fn slash_path()                     L737
// pub(super) fn to_root_relative()               L741
// fn rel_path_from_root()                        L746
// fn path_is_ignored()                           L751
// pub(super) struct WorkspaceMatch               L769
// pub(super) fn workspace_match()                L774
// fn split_package_specifier()                   L786
// pub(super) fn resolve_workspace_specifier()    L816
// fn resolve_workspace_entry()                   L829
// fn resolve_package_target()                    L881
// fn export_target()                             L892
// fn first_export_string()                       L920
// fn read_json_value()                           L936
// ---------------------------------------------------

pub type WorkspacePackages = HashMap<String, PathBuf>;

#[derive(Debug, Clone, Default)]
pub struct GoWorkspaceCache {
    pub modules_by_path: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct PythonWorkspaceCache {
    pub package_dirs: HashMap<String, Vec<PathBuf>>,
    pub module_files: HashMap<String, Vec<PathBuf>>,
}

#[derive(Debug, Clone, Default)]
pub struct RustWorkspaceCrate {
    pub name: String,
    pub src_root: PathBuf,
    pub crate_root_files: Vec<PathBuf>,
    pub dependency_src_roots: HashMap<String, PathBuf>,
    pub dependency_entry_files: HashMap<String, Vec<PathBuf>>,
}

#[derive(Debug, Clone, Default)]
pub struct RustWorkspaceCache {
    pub crates_by_root: HashMap<PathBuf, RustWorkspaceCrate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportKind {
    Relative,
    Workspace,
    TsconfigPath,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub raw: String,
    pub resolved_path: Option<PathBuf>,
    pub kind: ImportKind,
    pub names: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceImportIndex {
    pub workspace_packages: WorkspacePackages,
    pub go_workspace: GoWorkspaceCache,
    pub python_workspace: PythonWorkspaceCache,
    pub rust_workspace: RustWorkspaceCache,
    pub file_imports: BTreeMap<PathBuf, Vec<ResolvedImport>>,
}

pub fn build_workspace_import_index(
    root: &Path,
    ignore_patterns: &[String],
) -> Result<WorkspaceImportIndex> {
    let ignore_set = build_ignore_globset(ignore_patterns)?;
    let workspace_packages = discover_workspace_packages(root, &ignore_set);
    let go_workspace = go::build_workspace_cache(root);
    let python_workspace = python::build_workspace_cache(root, &ignore_set);
    let rust_workspace = rust::build_workspace_cache(root, &ignore_set);
    let mut file_imports = BTreeMap::new();

    for rel_path in discover_code_files(root, &ignore_set)? {
        let Some(language) = CodeLanguage::from_path(&rel_path) else {
            continue;
        };

        let abs_path = root.join(&rel_path);
        let source = match fs::read_to_string(&abs_path) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::InvalidData => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", abs_path.display()));
            }
        };

        let mut imports = resolve_imports_for_language(
            root,
            &rel_path,
            &source,
            language,
            &workspace_packages,
            &go_workspace,
            &python_workspace,
            &rust_workspace,
        );
        imports.sort_by(|left, right| {
            left.line
                .cmp(&right.line)
                .then_with(|| left.raw.cmp(&right.raw))
                .then_with(|| left.resolved_path.cmp(&right.resolved_path))
        });
        file_imports.insert(rel_path, imports);
    }

    Ok(WorkspaceImportIndex {
        workspace_packages,
        go_workspace,
        python_workspace,
        rust_workspace,
        file_imports,
    })
}

pub fn resolve_imports_for_language(
    root: &Path,
    source_file: &Path,
    source: &str,
    language: CodeLanguage,
    workspace_packages: &WorkspacePackages,
    go_workspace: &GoWorkspaceCache,
    python_workspace: &PythonWorkspaceCache,
    rust_workspace: &RustWorkspaceCache,
) -> Vec<ResolvedImport> {
    match language {
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            tsjs::resolve(root, source_file, source, workspace_packages)
        }
        CodeLanguage::Rust => rust::resolve(root, source_file, source, rust_workspace),
        CodeLanguage::Go => go::resolve(root, source_file, source, go_workspace),
        CodeLanguage::Python => python::resolve(root, source_file, source, python_workspace),
    }
}

const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    ".cache",
    "vendor",
    "__pycache__",
    ".venv",
    ".kdb",
];

const TSJS_EXTS: &[&str] = &[
    "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "d.ts", "d.mts", "d.cts",
];

fn build_ignore_globset(ignore_patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in ignore_patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("invalid ignore pattern `{pattern}`"))?;
        builder.add(glob);
    }

    builder.build().context("failed to compile ignore patterns")
}

fn discover_code_files(root: &Path, ignore_set: &GlobSet) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            let Some(rel) = rel_path_from_root(root, entry.path()) else {
                return false;
            };
            if rel.as_os_str().is_empty() {
                return true;
            }

            let name = entry.file_name().to_string_lossy();
            if IGNORED_DIRS.contains(&name.as_ref()) {
                return false;
            }

            !path_is_ignored(ignore_set, &rel, true)
        })
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let rel = entry.path().strip_prefix(root).with_context(|| {
            format!(
                "failed to strip root {} from {}",
                root.display(),
                entry.path().display()
            )
        })?;
        let Some(rel) = normalize_rel_path(rel) else {
            continue;
        };

        if path_is_ignored(ignore_set, &rel, false) {
            continue;
        }

        if CodeLanguage::from_path(&rel).is_some() {
            paths.push(rel);
        }
    }

    paths.sort();
    Ok(paths)
}

fn discover_workspace_packages(root: &Path, ignore_set: &GlobSet) -> WorkspacePackages {
    let patterns = workspace_patterns(root);
    let include_set = compile_globset(&patterns.includes);
    let exclude_set = compile_globset(&patterns.excludes);

    let mut packages = WorkspacePackages::new();
    let mut package_json_paths = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            let Some(rel) = rel_path_from_root(root, entry.path()) else {
                return false;
            };
            if rel.as_os_str().is_empty() {
                return true;
            }

            let name = entry.file_name().to_string_lossy();
            if IGNORED_DIRS.contains(&name.as_ref()) {
                return false;
            }

            !path_is_ignored(ignore_set, &rel, true)
        })
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        if entry.file_name().to_string_lossy() != "package.json" {
            continue;
        }

        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let Some(rel) = normalize_rel_path(rel) else {
            continue;
        };
        if path_is_ignored(ignore_set, &rel, false) {
            continue;
        }

        package_json_paths.push(rel);
    }

    package_json_paths.sort();

    for rel_manifest in package_json_paths {
        let rel_dir = rel_manifest.parent().unwrap_or(Path::new(""));
        if !workspace_path_allowed(rel_dir, include_set.as_ref(), exclude_set.as_ref()) {
            continue;
        }

        let Some(package_name) = package_name_from_manifest(root, &rel_manifest) else {
            continue;
        };

        let Some(package_root) = normalize_rel_path(rel_dir) else {
            continue;
        };
        packages.entry(package_name).or_insert(package_root);
    }

    packages
}

#[derive(Debug, Default)]
struct WorkspacePatternSet {
    includes: Vec<String>,
    excludes: Vec<String>,
}

fn workspace_patterns(root: &Path) -> WorkspacePatternSet {
    let mut patterns = WorkspacePatternSet::default();
    let mut all_patterns = Vec::new();
    all_patterns.extend(read_pnpm_workspace_patterns(root));
    all_patterns.extend(read_package_json_workspace_patterns(root));

    for pattern in all_patterns {
        let value = pattern.trim();
        if value.is_empty() {
            continue;
        }
        if let Some(rest) = value.strip_prefix('!') {
            let rest = rest.trim();
            if !rest.is_empty() {
                patterns.excludes.push(rest.to_string());
            }
            continue;
        }
        patterns.includes.push(value.to_string());
    }

    patterns
}

fn read_pnpm_workspace_patterns(root: &Path) -> Vec<String> {
    let path = root.join("pnpm-workspace.yaml");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(_) => return Vec::new(),
    };

    let mut patterns = Vec::new();
    let mut in_packages = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if !in_packages {
            if trimmed == "packages:" || trimmed.starts_with("packages:") {
                in_packages = true;
            }
            continue;
        }

        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        if indent == 0 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            in_packages = false;
            continue;
        }

        let Some(item) = trimmed.strip_prefix('-') else {
            if indent == 0 {
                in_packages = false;
            }
            continue;
        };

        let value = item.trim().trim_matches('"').trim_matches('\'').trim();
        if !value.is_empty() {
            patterns.push(value.to_string());
        }
    }

    patterns
}

fn read_package_json_workspace_patterns(root: &Path) -> Vec<String> {
    let path = root.join("package.json");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(_) => return Vec::new(),
    };

    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(workspaces) = value.get("workspaces") else {
        return Vec::new();
    };

    if let Some(array) = workspaces.as_array() {
        return array
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect();
    }

    workspaces
        .as_object()
        .and_then(|table| table.get("packages"))
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn compile_globset(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let Ok(glob) = GlobBuilder::new(pattern).literal_separator(true).build() else {
            continue;
        };
        builder.add(glob);
    }

    builder.build().ok()
}

fn workspace_path_allowed(
    rel_dir: &Path,
    include_set: Option<&GlobSet>,
    exclude_set: Option<&GlobSet>,
) -> bool {
    let slash = slash_path(rel_dir);

    if include_set.is_some_and(|set| !globset_matches_path(set, &slash)) {
        return false;
    }

    if exclude_set.is_some_and(|set| globset_matches_path(set, &slash)) {
        return false;
    }

    true
}

fn globset_matches_path(set: &GlobSet, slash_path: &str) -> bool {
    if set.is_match(slash_path) {
        return true;
    }

    if slash_path.is_empty() {
        return set.is_match("./");
    }

    set.is_match(format!("{slash_path}/"))
}

fn package_name_from_manifest(root: &Path, rel_manifest: &Path) -> Option<String> {
    let abs = root.join(rel_manifest);
    let raw = fs::read_to_string(abs).ok()?;
    let value = serde_json::from_str::<Value>(&raw).ok()?;
    let name = value.get("name")?.as_str()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub(super) fn classify_local_kind(
    specifier: &str,
    workspace_packages: &WorkspacePackages,
) -> ImportKind {
    if specifier.starts_with('.') || specifier.starts_with('/') {
        return ImportKind::Relative;
    }

    if workspace_match(specifier, workspace_packages).is_some() {
        return ImportKind::Workspace;
    }

    ImportKind::TsconfigPath
}

pub(super) fn sanitize_specifier(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let no_query = trimmed.split('?').next().unwrap_or(trimmed);
    let no_fragment = no_query.split('#').next().unwrap_or(no_query);
    let value = no_fragment.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub(super) fn normalize_identifier(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    let value = value.trim_start_matches("type ").trim();
    let value = value.trim_start_matches('_').trim();
    if value.is_empty() {
        return None;
    }

    let value = value
        .split_whitespace()
        .next()
        .unwrap_or(value)
        .trim_matches(',')
        .trim_matches(';')
        .trim_matches('}')
        .trim_matches('{')
        .trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

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
    if !root.join(&rel).is_file() {
        return None;
    }

    Some(canonicalize_existing_rel_path(root, &rel))
}

fn canonicalize_existing_rel_path(root: &Path, rel: &Path) -> PathBuf {
    let mut canonical = PathBuf::new();
    let mut cursor = root.to_path_buf();

    for component in rel.components() {
        let Component::Normal(part) = component else {
            continue;
        };

        let mut exact_name = None;
        let mut folded_name = None;
        let part_text = part.to_string_lossy();

        if let Ok(entries) = fs::read_dir(&cursor) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name == part {
                    exact_name = Some(name);
                    break;
                }

                if folded_name.is_none() && name.to_string_lossy().eq_ignore_ascii_case(&part_text)
                {
                    folded_name = Some(name);
                }
            }
        }

        let selected = exact_name
            .or(folded_name)
            .unwrap_or_else(|| part.to_os_string());
        cursor.push(&selected);
        canonical.push(selected);
    }

    canonical
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

pub(super) fn build_line_starts(content: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

pub(super) fn line_number_for_offset(line_starts: &[usize], byte_index: usize) -> usize {
    let line_idx = match line_starts.binary_search(&byte_index) {
        Ok(index) => index,
        Err(0) => 0,
        Err(index) => index - 1,
    };
    line_idx + 1
}

pub(super) fn normalize_rel_path(path: &Path) -> Option<PathBuf> {
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

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn to_root_relative(root: &Path, abs_path: &Path) -> Option<PathBuf> {
    let rel = abs_path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}

fn rel_path_from_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}

fn path_is_ignored(ignore_set: &GlobSet, rel_path: &Path, is_dir: bool) -> bool {
    let slash = slash_path(rel_path);
    if slash.is_empty() {
        return false;
    }

    if ignore_set.is_match(&slash) {
        return true;
    }

    if is_dir {
        return ignore_set.is_match(format!("{slash}/"));
    }

    false
}

#[derive(Debug, Clone)]
pub(super) struct WorkspaceMatch {
    pub package_root: PathBuf,
    pub subpath: Option<String>,
}

pub(super) fn workspace_match(
    specifier: &str,
    workspace_packages: &WorkspacePackages,
) -> Option<WorkspaceMatch> {
    let (package_name, subpath) = split_package_specifier(specifier)?;
    let package_root = workspace_packages.get(&package_name)?.clone();
    Some(WorkspaceMatch {
        package_root,
        subpath,
    })
}

fn split_package_specifier(specifier: &str) -> Option<(String, Option<String>)> {
    let value = specifier.trim();
    if value.is_empty() {
        return None;
    }

    let mut segments = value.split('/');
    let first = segments.next()?;
    if first.is_empty() {
        return None;
    }

    if first.starts_with('@') {
        let second = segments.next()?;
        if second.is_empty() {
            return None;
        }

        let package_name = format!("{first}/{second}");
        let rest = segments.collect::<Vec<_>>().join("/");
        let subpath = (!rest.is_empty()).then_some(rest);
        return Some((package_name, subpath));
    }

    let package_name = first.to_string();
    let rest = segments.collect::<Vec<_>>().join("/");
    let subpath = (!rest.is_empty()).then_some(rest);
    Some((package_name, subpath))
}

pub(super) fn resolve_workspace_specifier(
    root: &Path,
    specifier: &str,
    workspace_packages: &WorkspacePackages,
) -> Option<PathBuf> {
    let workspace_match = workspace_match(specifier, workspace_packages)?;
    resolve_workspace_entry(
        root,
        &workspace_match.package_root,
        workspace_match.subpath.as_deref(),
    )
}

fn resolve_workspace_entry(
    root: &Path,
    package_root: &Path,
    subpath: Option<&str>,
) -> Option<PathBuf> {
    let manifest = read_json_value(root.join(package_root).join("package.json"));

    if let Some(subpath) = subpath {
        if let Some(target) = manifest
            .as_ref()
            .and_then(|value| value.get("exports"))
            .and_then(|exports| export_target(exports, &format!("./{subpath}")))
            .and_then(|target| resolve_package_target(root, package_root, &target))
        {
            return Some(target);
        }

        let candidate = package_root.join(subpath);
        return resolve_with_exts(root, &candidate, TSJS_EXTS)
            .or_else(|| resolve_file(root, &candidate));
    }

    if let Some(target) = manifest
        .as_ref()
        .and_then(|value| value.get("exports"))
        .and_then(|exports| export_target(exports, "."))
        .and_then(|target| resolve_package_target(root, package_root, &target))
    {
        return Some(target);
    }

    if let Some(value) = manifest.as_ref() {
        for field in ["types", "typings", "module", "main"] {
            if let Some(target) = value
                .get(field)
                .and_then(Value::as_str)
                .and_then(|target| resolve_package_target(root, package_root, target))
            {
                return Some(target);
            }
        }
    }

    for base in [package_root.join("src/index"), package_root.join("index")] {
        if let Some(path) = resolve_with_exts(root, &base, TSJS_EXTS) {
            return Some(path);
        }
    }

    None
}

fn resolve_package_target(root: &Path, package_root: &Path, raw_target: &str) -> Option<PathBuf> {
    let target = sanitize_specifier(raw_target)?;
    let rel = Path::new(target.trim_start_matches("./"));
    if rel.as_os_str().is_empty() || rel.is_absolute() {
        return None;
    }

    let candidate = package_root.join(rel);
    resolve_with_exts(root, &candidate, TSJS_EXTS).or_else(|| resolve_file(root, &candidate))
}

fn export_target(exports: &Value, key: &str) -> Option<String> {
    match exports {
        Value::String(value) => {
            if key == "." {
                Some(value.to_string())
            } else {
                None
            }
        }
        Value::Array(values) => values.iter().find_map(|value| export_target(value, key)),
        Value::Object(table) => {
            if let Some(value) = table.get(key) {
                return first_export_string(value);
            }

            if key == "." {
                let has_subpath_keys = table.keys().any(|entry| entry.starts_with('.'));
                if !has_subpath_keys {
                    return first_export_string(exports);
                }
            }

            None
        }
        _ => None,
    }
}

fn first_export_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.to_string()),
        Value::Array(values) => values.iter().find_map(first_export_string),
        Value::Object(table) => {
            for key in ["types", "import", "module", "default", "require"] {
                if let Some(value) = table.get(key).and_then(first_export_string) {
                    return Some(value);
                }
            }
            table.values().find_map(first_export_string)
        }
        _ => None,
    }
}

fn read_json_value(path: PathBuf) -> Option<Value> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&raw).ok()
}
