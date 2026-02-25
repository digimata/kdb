//! Workspace-aware code import resolution.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
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

pub use go::GoWorkspaceCache;
pub use python::PythonWorkspaceCache;
pub use rust::{RustWorkspaceCache, RustWorkspaceCrate};

// ------------------------------------------------
// src/resolve/mod.rs
//
// mod go                                       L13
// mod python                                   L14
// mod rust                                     L15
// mod tsjs                                     L16
// pub type WorkspacePackages                   L57
// pub struct WorkspaceCaches                   L60
//   pub fn build()                             L68
// pub enum ImportKind                          L80
// pub struct ResolvedImport                    L88
// pub struct WorkspaceImportIndex              L97
// pub fn build_workspace_import_index()       L105
// pub fn resolve_imports_for_language()       L148
// pub(crate) fn collect_tsjs_specifiers()     L174
// pub(crate) fn collect_rust_mod_and_use()    L178
// const IGNORED_DIRS                          L182
// fn build_ignore_globset()                   L196
// fn discover_code_files()                    L209
// pub(super) fn sanitize_specifier()          L264
// pub(super) fn normalize_identifier()        L281
// pub(super) fn resolve_with_exts()           L309
// pub(super) fn resolve_file()                L335
// fn canonicalize_existing_rel_path()         L344
// pub(super) fn list_go_package_files()       L382
// pub(super) fn build_line_starts()           L416
// pub(super) fn line_number_for_offset()      L426
// pub(super) fn normalize_rel_path()          L435
// pub(super) fn slash_path()                  L454
// pub(super) fn to_root_relative()            L458
// fn rel_path_from_root()                     L463
// fn path_is_ignored()                        L468
// ------------------------------------------------

pub type WorkspacePackages = HashMap<String, PathBuf>;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceCaches {
    pub workspace_packages: WorkspacePackages,
    pub go_workspace: GoWorkspaceCache,
    pub python_workspace: PythonWorkspaceCache,
    pub rust_workspace: RustWorkspaceCache,
}

impl WorkspaceCaches {
    pub fn build(root: &Path, ignore_patterns: &[String]) -> Result<Self> {
        let ignore_set = build_ignore_globset(ignore_patterns)?;
        Ok(Self {
            workspace_packages: tsjs::discover_workspace_packages(root, &ignore_set),
            go_workspace: GoWorkspaceCache::build(root),
            python_workspace: PythonWorkspaceCache::build(root, &ignore_set),
            rust_workspace: RustWorkspaceCache::build(root, &ignore_set),
        })
    }
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
    let workspace_caches = WorkspaceCaches::build(root, ignore_patterns)?;
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

        let mut imports =
            resolve_imports_for_language(root, &rel_path, &source, language, &workspace_caches);
        imports.sort_by(|left, right| {
            left.line
                .cmp(&right.line)
                .then_with(|| left.raw.cmp(&right.raw))
                .then_with(|| left.resolved_path.cmp(&right.resolved_path))
        });
        file_imports.insert(rel_path, imports);
    }

    Ok(WorkspaceImportIndex {
        workspace_packages: workspace_caches.workspace_packages,
        go_workspace: workspace_caches.go_workspace,
        python_workspace: workspace_caches.python_workspace,
        rust_workspace: workspace_caches.rust_workspace,
        file_imports,
    })
}

pub fn resolve_imports_for_language(
    root: &Path,
    source_file: &Path,
    source: &str,
    language: CodeLanguage,
    workspace_caches: &WorkspaceCaches,
) -> Vec<ResolvedImport> {
    match language {
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            let resolver = tsjs::TsjsResolver::new(root, &workspace_caches.workspace_packages);
            resolver.resolve(source_file, source)
        }
        CodeLanguage::Rust => workspace_caches
            .rust_workspace
            .resolve(root, source_file, source),
        CodeLanguage::Go => workspace_caches
            .go_workspace
            .resolve(root, source_file, source),
        CodeLanguage::Python => {
            workspace_caches
                .python_workspace
                .resolve(root, source_file, source)
        }
    }
}

pub(crate) fn collect_tsjs_specifiers(source_file: &Path, source: &str) -> Vec<String> {
    tsjs::collect_specifiers(source_file, source)
}

pub(crate) fn collect_rust_mod_and_use(source: &str) -> (Vec<String>, Vec<String>) {
    rust::collect_mod_and_use(source)
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
