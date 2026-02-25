use globset::GlobSet;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use toml::Value as TomlValue;
use walkdir::WalkDir;

use super::{
    normalize_identifier, normalize_rel_path, resolve_file, to_root_relative, ImportKind,
    PythonWorkspaceCache, ResolvedImport,
};

// ---------------------------------------------
// src/resolve/python.rs
//
// static FIND_PACKAGES_WHERE_RE             L46
// static PACKAGE_DIR_RE                     L51
// pub(super) fn build_workspace_cache()     L56
// pub(super) fn resolve()                   L77
// fn push_import_statement()               L120
// fn push_from_import_statement()          L151
// fn discover_python_project_roots()       L206
// fn project_package_roots()               L255
// fn pyproject_package_roots()             L277
// fn setup_py_package_roots()              L295
// fn collect_setuptools_roots()            L318
// fn collect_poetry_roots()                L358
// fn collect_hatch_roots()                 L383
// fn push_project_root()                   L412
// fn index_package_root()                  L436
// fn resolve_module()                      L477
// fn module_paths()                        L491
// fn relative_module_path()                L511
// fn absolute_module_paths()               L527
// fn resolve_module_path()                 L557
// fn parse_names()                         L566
// fn split_alias()                         L597
// fn module_binding_name()                 L603
// fn classify_kind()                       L613
// fn has_python_top_level_entries()        L625
// fn is_python_source()                    L644
// fn is_python_package_dir()               L652
// ---------------------------------------------

static FIND_PACKAGES_WHERE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"find_(?:namespace_)?packages\s*\(\s*where\s*=\s*["']([^"']+)["']"#)
        .expect("valid setup.py find_packages(where=...) regex")
});

static PACKAGE_DIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"package_dir\s*=\s*\{\s*["']\s*["']\s*:\s*["']([^"']+)["']"#)
        .expect("valid setup.py package_dir regex")
});

pub(super) fn build_workspace_cache(root: &Path, ignore_set: &GlobSet) -> PythonWorkspaceCache {
    let mut cache = PythonWorkspaceCache::default();

    for project_root in discover_python_project_roots(root, ignore_set) {
        for package_root in project_package_roots(root, &project_root) {
            index_package_root(root, &package_root, &mut cache);
        }
    }

    for paths in cache.package_dirs.values_mut() {
        paths.sort();
        paths.dedup();
    }
    for paths in cache.module_files.values_mut() {
        paths.sort();
        paths.dedup();
    }

    cache
}

pub(super) fn resolve(
    root: &Path,
    source_file: &Path,
    source: &str,
    python_workspace: &PythonWorkspaceCache,
) -> Vec<ResolvedImport> {
    let mut imports = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_no = index + 1;
        let no_comment = line.split('#').next().unwrap_or(line).trim();
        if no_comment.is_empty() {
            continue;
        }

        if let Some(rest) = no_comment.strip_prefix("import ") {
            push_import_statement(
                root,
                source_file,
                python_workspace,
                line_no,
                rest,
                &mut imports,
            );
            continue;
        }

        let Some(rest) = no_comment.strip_prefix("from ") else {
            continue;
        };
        push_from_import_statement(
            root,
            source_file,
            python_workspace,
            line_no,
            rest,
            &mut imports,
        );
    }

    imports
}

fn push_import_statement(
    root: &Path,
    source_file: &Path,
    python_workspace: &PythonWorkspaceCache,
    line_no: usize,
    rest: &str,
    imports: &mut Vec<ResolvedImport>,
) {
    for module in rest.split(',') {
        let item = module.trim();
        if item.is_empty() {
            continue;
        }

        let (module_name, alias) = split_alias(item);
        let resolved_path = resolve_module(root, source_file, module_name, python_workspace);
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
}

fn push_from_import_statement(
    root: &Path,
    source_file: &Path,
    python_workspace: &PythonWorkspaceCache,
    line_no: usize,
    rest: &str,
    imports: &mut Vec<ResolvedImport>,
) {
    let Some((module, imported)) = rest.split_once(" import ") else {
        return;
    };

    let module = module.trim();
    if module.chars().any(|ch| ch != '.') {
        let resolved_path = resolve_module(root, source_file, module, python_workspace);
        let kind = classify_kind(module, resolved_path.is_some());
        imports.push(ResolvedImport {
            raw: module.to_string(),
            resolved_path,
            kind,
            names: Vec::new(),
            line: line_no,
        });
    }

    let module_paths = module_paths(source_file, module, python_workspace);
    for (name, local_name) in parse_names(imported) {
        if name == "*" {
            continue;
        }

        let mut resolved_path = None;
        for module_path in &module_paths {
            let candidate = module_path.join(name.replace('.', "/"));
            resolved_path = resolve_module_path(root, &candidate);
            if resolved_path.is_some() {
                break;
            }
        }

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

fn discover_python_project_roots(root: &Path, ignore_set: &GlobSet) -> Vec<PathBuf> {
    let mut roots = Vec::new();

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

        let name = entry.file_name().to_string_lossy();
        if name != "pyproject.toml" && name != "setup.py" {
            continue;
        }

        let Some(rel) = to_root_relative(root, entry.path()) else {
            continue;
        };
        let project_root = rel.parent().unwrap_or(Path::new("")).to_path_buf();
        if roots.iter().all(|existing| existing != &project_root) {
            roots.push(project_root);
        }
    }

    roots.sort();
    roots
}

fn project_package_roots(root: &Path, project_root: &Path) -> Vec<PathBuf> {
    let mut roots = pyproject_package_roots(root, project_root);
    let setup_roots = setup_py_package_roots(root, project_root);
    for setup_root in setup_roots {
        if roots.iter().all(|existing| existing != &setup_root) {
            roots.push(setup_root);
        }
    }

    if roots.is_empty() {
        let src_root = project_root.join("src");
        if has_python_top_level_entries(root, &src_root) {
            roots.push(src_root);
        } else {
            roots.push(project_root.to_path_buf());
        }
    }

    roots.sort();
    roots
}

fn pyproject_package_roots(root: &Path, project_root: &Path) -> Vec<PathBuf> {
    let path = root.join(project_root).join("pyproject.toml");
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(_) => return Vec::new(),
    };
    let value = match toml::from_str::<TomlValue>(&source) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let mut roots = Vec::new();
    collect_setuptools_roots(root, project_root, &value, &mut roots);
    collect_poetry_roots(root, project_root, &value, &mut roots);
    collect_hatch_roots(root, project_root, &value, &mut roots);
    roots
}

fn setup_py_package_roots(root: &Path, project_root: &Path) -> Vec<PathBuf> {
    let path = root.join(project_root).join("setup.py");
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(_) => return Vec::new(),
    };

    let mut roots = Vec::new();
    for captures in FIND_PACKAGES_WHERE_RE.captures_iter(&source) {
        let Some(raw) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        push_project_root(root, project_root, raw, &mut roots);
    }
    for captures in PACKAGE_DIR_RE.captures_iter(&source) {
        let Some(raw) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        push_project_root(root, project_root, raw, &mut roots);
    }
    roots
}

fn collect_setuptools_roots(
    root: &Path,
    project_root: &Path,
    value: &TomlValue,
    roots: &mut Vec<PathBuf>,
) {
    let Some(setuptools) = value
        .get("tool")
        .and_then(TomlValue::as_table)
        .and_then(|tool| tool.get("setuptools"))
        .and_then(TomlValue::as_table)
    else {
        return;
    };

    if let Some(package_dir) = setuptools.get("package-dir").and_then(TomlValue::as_table) {
        if let Some(raw) = package_dir.get("").and_then(TomlValue::as_str) {
            push_project_root(root, project_root, raw, roots);
        }
    }

    let Some(find) = setuptools
        .get("packages")
        .and_then(TomlValue::as_table)
        .and_then(|packages| packages.get("find"))
        .and_then(TomlValue::as_table)
    else {
        return;
    };

    if let Some(raw) = find.get("where").and_then(TomlValue::as_str) {
        push_project_root(root, project_root, raw, roots);
    }
    if let Some(entries) = find.get("where").and_then(TomlValue::as_array) {
        for entry in entries.iter().filter_map(TomlValue::as_str) {
            push_project_root(root, project_root, entry, roots);
        }
    }
}

fn collect_poetry_roots(
    root: &Path,
    project_root: &Path,
    value: &TomlValue,
    roots: &mut Vec<PathBuf>,
) {
    let Some(packages) = value
        .get("tool")
        .and_then(TomlValue::as_table)
        .and_then(|tool| tool.get("poetry"))
        .and_then(TomlValue::as_table)
        .and_then(|poetry| poetry.get("packages"))
        .and_then(TomlValue::as_array)
    else {
        return;
    };

    for package in packages.iter().filter_map(TomlValue::as_table) {
        let Some(raw) = package.get("from").and_then(TomlValue::as_str) else {
            continue;
        };
        push_project_root(root, project_root, raw, roots);
    }
}

fn collect_hatch_roots(
    root: &Path,
    project_root: &Path,
    value: &TomlValue,
    roots: &mut Vec<PathBuf>,
) {
    let Some(packages) = value
        .get("tool")
        .and_then(TomlValue::as_table)
        .and_then(|tool| tool.get("hatch"))
        .and_then(TomlValue::as_table)
        .and_then(|hatch| hatch.get("build"))
        .and_then(TomlValue::as_table)
        .and_then(|build| build.get("targets"))
        .and_then(TomlValue::as_table)
        .and_then(|targets| targets.get("wheel"))
        .and_then(TomlValue::as_table)
        .and_then(|wheel| wheel.get("packages"))
        .and_then(TomlValue::as_array)
    else {
        return;
    };

    for package in packages.iter().filter_map(TomlValue::as_str) {
        let parent = Path::new(package).parent().unwrap_or(Path::new(""));
        push_project_root(root, project_root, &parent.to_string_lossy(), roots);
    }
}

fn push_project_root(root: &Path, project_root: &Path, raw: &str, roots: &mut Vec<PathBuf>) {
    let token = raw.trim().trim_matches('"').trim_matches('`').trim();
    if token.is_empty() {
        return;
    }

    let candidate = Path::new(token);
    let rel = if candidate.is_absolute() {
        to_root_relative(root, candidate)
    } else {
        normalize_rel_path(&project_root.join(candidate))
    };
    let Some(rel) = rel else {
        return;
    };
    if !root.join(&rel).is_dir() {
        return;
    }

    if roots.iter().all(|existing| existing != &rel) {
        roots.push(rel);
    }
}

fn index_package_root(root: &Path, package_root: &Path, cache: &mut PythonWorkspaceCache) {
    let abs_root = root.join(package_root);
    let Ok(entries) = fs::read_dir(abs_root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !is_python_package_dir(&path) {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let Some(name) = normalize_identifier(&name) else {
                continue;
            };
            let Some(rel) = to_root_relative(root, &path) else {
                continue;
            };
            cache.package_dirs.entry(name).or_default().push(rel);
            continue;
        }

        if !is_python_source(&path) || path.ends_with("__init__.py") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(name) = normalize_identifier(stem) else {
            continue;
        };
        let Some(rel) = to_root_relative(root, &path) else {
            continue;
        };
        cache.module_files.entry(name).or_default().push(rel);
    }
}

fn resolve_module(
    root: &Path,
    source_file: &Path,
    module: &str,
    python_workspace: &PythonWorkspaceCache,
) -> Option<PathBuf> {
    for module_path in module_paths(source_file, module, python_workspace) {
        if let Some(path) = resolve_module_path(root, &module_path) {
            return Some(path);
        }
    }
    None
}

fn module_paths(
    source_file: &Path,
    module: &str,
    python_workspace: &PythonWorkspaceCache,
) -> Vec<PathBuf> {
    if module.starts_with('.') {
        return relative_module_path(source_file, module)
            .into_iter()
            .collect();
    }

    let mut paths = absolute_module_paths(module, python_workspace);
    if let Some(fallback) = normalize_rel_path(Path::new(&module.replace('.', "/"))) {
        if paths.iter().all(|existing| existing != &fallback) {
            paths.push(fallback);
        }
    }
    paths
}

fn relative_module_path(source_file: &Path, module: &str) -> Option<PathBuf> {
    let dots = module.chars().take_while(|ch| *ch == '.').count();
    let tail = module[dots..].trim();
    let mut base = source_file.parent().unwrap_or(Path::new("")).to_path_buf();

    for _ in 1..dots {
        base = base.parent()?.to_path_buf();
    }

    if tail.is_empty() {
        return normalize_rel_path(&base);
    }

    normalize_rel_path(&base.join(tail.replace('.', "/")))
}

fn absolute_module_paths(module: &str, python_workspace: &PythonWorkspaceCache) -> Vec<PathBuf> {
    let mut parts = module.split('.').filter(|part| !part.is_empty());
    let Some(head) = parts.next() else {
        return Vec::new();
    };

    let tail = parts.collect::<Vec<_>>();
    let mut paths = Vec::new();

    if let Some(package_dirs) = python_workspace.package_dirs.get(head) {
        for package_dir in package_dirs {
            let mut candidate = package_dir.clone();
            for part in &tail {
                candidate.push(part);
            }
            paths.push(candidate);
        }
    }

    if tail.is_empty() {
        if let Some(module_files) = python_workspace.module_files.get(head) {
            for module_file in module_files {
                paths.push(module_file.with_extension(""));
            }
        }
    }

    paths
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

fn split_alias(item: &str) -> (&str, Option<&str>) {
    item.split_once(" as ")
        .map(|(name, alias)| (name.trim(), Some(alias.trim())))
        .unwrap_or((item.trim(), None))
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

fn has_python_top_level_entries(root: &Path, package_root: &Path) -> bool {
    let abs_root = root.join(package_root);
    let Ok(entries) = fs::read_dir(abs_root) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && is_python_package_dir(&path) {
            return true;
        }
        if is_python_source(&path) && !path.ends_with("__init__.py") {
            return true;
        }
    }

    false
}

fn is_python_source(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
}

fn is_python_package_dir(path: &Path) -> bool {
    if path.join("__init__.py").is_file() {
        return true;
    }

    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .any(|entry| is_python_source(entry.path()))
}
