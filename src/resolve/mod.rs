//! Workspace-aware code import resolution.

use anyhow::{Context, Result};
use globset::GlobSet;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

use crate::lang::CodeLanguage;
// NOTE: index block managed by kdb fmt — do not update manually

pub(super) use crate::project::ignore::ALWAYS_IGNORED_DIRS;
pub(super) use crate::project::ignore::{build_ignore_globset, path_is_ignored};
pub(super) use crate::project::paths::normalize_rel_path;

mod go;
mod python;
mod rust;
mod tsjs;

pub use go::GoWorkspaceCache;
pub use python::PythonWorkspaceCache;
pub(crate) use rust::collect_mod_and_use;
pub use rust::{RustWorkspaceCache, RustWorkspaceCrate};
pub(crate) use tsjs::collect_specifiers;

pub(crate) use go::GoResolver;
pub(crate) use python::PythonResolver;
pub(crate) use rust::RustResolver;
pub(crate) use tsjs::TsjsResolver;

/// Map from workspace package name to its root directory (relative to project root).
pub type WorkspacePackages = HashMap<String, PathBuf>;

/// Pre-built caches for each language's workspace resolution.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceCaches {
    pub workspace_packages: WorkspacePackages,
    pub go_workspace: GoWorkspaceCache,
    pub python_workspace: PythonWorkspaceCache,
    pub rust_workspace: RustWorkspaceCache,
}

impl WorkspaceCaches {
    /// Build all language workspace caches for the given project root.
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

/// Classification of an import's relationship to the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportKind {
    Relative,
    Workspace,
    TsconfigPath,
    External,
}

/// A single resolved import: the raw specifier, its resolved file path (if
/// local), its kind, the names it introduces, and its source line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub raw: String,
    pub resolved_path: Option<PathBuf>,
    pub kind: ImportKind,
    pub names: Vec<String>,
    pub line: usize,
}

/// Contract for language-specific import resolution.
///
/// Each language resolver takes a source file path and its contents, and
/// returns the list of resolved imports found in that file. Dispatch remains
/// static (no `Box<dyn>`); the trait enforces a uniform API across languages.
pub(crate) trait LanguageResolver {
    /// Resolve all imports in `source` as seen from `source_file`.
    fn resolve(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport>;
}

/// Result of scanning all code files for imports.
///
/// Internal to the resolve module; consumed by [`crate::index::CodeIndex::build`].
#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceImportResult {
    pub workspace_packages: WorkspacePackages,
    pub go_workspace: GoWorkspaceCache,
    pub rust_workspace: RustWorkspaceCache,
    pub file_imports: BTreeMap<PathBuf, Vec<ResolvedImport>>,
}

/// Scan every code file under `root` and resolve all imports.
pub(crate) fn build_workspace_import_index(
    root: &Path,
    ignore_patterns: &[String],
) -> Result<WorkspaceImportResult> {
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

    Ok(WorkspaceImportResult {
        workspace_packages: workspace_caches.workspace_packages,
        go_workspace: workspace_caches.go_workspace,
        rust_workspace: workspace_caches.rust_workspace,
        file_imports,
    })
}

/// Resolve all imports in a single source file, dispatching to the appropriate
/// language-specific resolver.
pub fn resolve_imports_for_language(
    root: &Path,
    source_file: &Path,
    source: &str,
    language: CodeLanguage,
    workspace_caches: &WorkspaceCaches,
) -> Vec<ResolvedImport> {
    match language {
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            let resolver = TsjsResolver::new(root, &workspace_caches.workspace_packages);
            resolver.resolve(source_file, source)
        }
        CodeLanguage::Rust => {
            let resolver = RustResolver::new(root, &workspace_caches.rust_workspace);
            resolver.resolve(source_file, source)
        }
        CodeLanguage::Go => {
            let resolver = GoResolver::new(root, &workspace_caches.go_workspace);
            resolver.resolve(source_file, source)
        }
        CodeLanguage::Python => {
            let resolver = PythonResolver::new(root, &workspace_caches.python_workspace);
            resolver.resolve(source_file, source)
        }
    }
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
            if ALWAYS_IGNORED_DIRS.contains(&name.as_ref()) {
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

/// Strip query strings and fragment hashes from a specifier, returning `None`
/// if the result is empty.
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

/// Normalize a raw binding name: strip `type ` prefixes, leading underscores,
/// trailing punctuation, and return `None` if nothing remains.
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

/// Try to resolve `base` as a file, appending each extension in `exts` and
/// also checking for `index.<ext>` directories.
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

/// Resolve a candidate relative path to a file, returning the case-corrected
/// relative path if the file exists.
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

/// List all `.go` files in a directory, returning sorted relative paths.
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

/// Convert a path to a forward-slash string for glob matching.
pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Strip `root` from an absolute path and normalize the remainder.
pub(super) fn to_root_relative(root: &Path, abs_path: &Path) -> Option<PathBuf> {
    let rel = abs_path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}

fn rel_path_from_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}
