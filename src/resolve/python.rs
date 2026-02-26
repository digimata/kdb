use globset::GlobSet;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value as TomlValue;
use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

use crate::symbols::walk_depth_first;

use super::{
    ImportKind, ImportNames, LanguageResolver, ResolvedImport, normalize_identifier,
    normalize_rel_path, resolve_file, to_root_relative,
};

// ------------------------------------------
// src/resolve/python.rs
//
// pub struct PythonWorkspaceCache        L65
//   pub(super) fn build()                L73
// pub(crate) struct PythonResolver       L96
//   pub(super) fn new()                 L103
//   fn resolve()                        L110
// struct PythonImportResolver           L115
//   fn new()                            L122
//   fn resolve_source()                 L130
//   fn push_import_statement()          L154
//   fn push_from_import_statement()     L178
//   fn resolve_module()                 L231
//   fn module_paths()                   L240
//   fn relative_module_path()           L254
//   fn absolute_module_paths()          L274
//   fn resolve_module_path()            L304
// fn discover_python_project_roots()    L314
// fn project_package_roots()            L363
// fn pyproject_package_roots()          L385
// fn setup_py_package_roots()           L403
// fn setup_py_root_tokens()             L418
// struct SetupPyRootCollector           L424
//   fn new()                            L430
//   fn collect()                        L441
//   fn call_function_name()             L478
//   fn keyword_argument_value()         L489
//   fn call_keyword_string_arg()        L518
//   fn setup_package_dir_root()         L529
//   fn dictionary_root_value()          L534
//   fn python_string_literal()          L570
// fn collect_setuptools_roots()         L605
// fn collect_poetry_roots()             L645
// fn collect_hatch_roots()              L670
// fn push_project_root()                L699
// fn index_package_root()               L723
// fn parse_names()                      L764
// fn split_alias()                      L795
// fn module_binding_name()              L801
// fn classify_kind()                    L811
// fn has_python_top_level_entries()     L823
// fn is_python_source()                 L842
// fn is_python_package_dir()            L850
// ------------------------------------------

/// Cached Python workspace data: maps top-level package names and module
/// stems to their directory and file paths.
#[derive(Debug, Clone, Default)]
pub struct PythonWorkspaceCache {
    pub package_dirs: HashMap<String, Vec<PathBuf>>,
    pub module_files: HashMap<String, Vec<PathBuf>>,
}

impl PythonWorkspaceCache {
    /// Build the workspace cache by discovering Python project roots and
    /// indexing their packages.
    pub(super) fn build(root: &Path, ignore_set: &GlobSet) -> Self {
        let mut cache = Self::default();

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
}

/// Resolves Python import statements against the workspace cache.
pub(crate) struct PythonResolver<'a> {
    root: &'a Path,
    workspace: &'a PythonWorkspaceCache,
}

impl<'a> PythonResolver<'a> {
    /// Create a new resolver for the given project root and workspace cache.
    pub(super) fn new(root: &'a Path, workspace: &'a PythonWorkspaceCache) -> Self {
        Self { root, workspace }
    }
}

impl LanguageResolver for PythonResolver<'_> {
    /// Resolve all Python imports in the given source file.
    fn resolve(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
        PythonImportResolver::new(self.root, source_file, self.workspace).resolve_source(source)
    }
}

struct PythonImportResolver<'a> {
    root: &'a Path,
    source_file: &'a Path,
    workspace: &'a PythonWorkspaceCache,
}

impl<'a> PythonImportResolver<'a> {
    fn new(root: &'a Path, source_file: &'a Path, workspace: &'a PythonWorkspaceCache) -> Self {
        Self {
            root,
            source_file,
            workspace,
        }
    }

    fn resolve_source(&self, source: &str) -> Vec<ResolvedImport> {
        let mut imports = Vec::new();

        for (index, line) in source.lines().enumerate() {
            let line_no = index + 1;
            let no_comment = line.split('#').next().unwrap_or(line).trim();
            if no_comment.is_empty() {
                continue;
            }

            if let Some(rest) = no_comment.strip_prefix("import ") {
                self.push_import_statement(line_no, rest, &mut imports);
                continue;
            }

            let Some(rest) = no_comment.strip_prefix("from ") else {
                continue;
            };
            self.push_from_import_statement(line_no, rest, &mut imports);
        }

        imports
    }

    fn push_import_statement(&self, line_no: usize, rest: &str, imports: &mut Vec<ResolvedImport>) {
        for module in rest.split(',') {
            let item = module.trim();
            if item.is_empty() {
                continue;
            }

            let (module_name, alias) = split_alias(item);
            let resolved_path = self.resolve_module(module_name);
            let kind = classify_kind(module_name, resolved_path.is_some());
            let local = module_binding_name(module_name, alias);
            let def = module_binding_name(module_name, None);

            let mut names = ImportNames::new(local.clone().into_iter().collect());
            if let (Some(local), Some(def)) = (&local, &def) {
                if local != def {
                    names.aliases.insert(local.clone(), def.clone());
                }
            }

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
        &self,
        line_no: usize,
        rest: &str,
        imports: &mut Vec<ResolvedImport>,
    ) {
        let Some((module, imported)) = rest.split_once(" import ") else {
            return;
        };

        let module = module.trim();
        let parent_resolved = if module.chars().any(|ch| ch != '.') {
            let resolved_path = self.resolve_module(module);
            let kind = classify_kind(module, resolved_path.is_some());
            imports.push(ResolvedImport {
                raw: module.to_string(),
                resolved_path: resolved_path.clone(),
                kind,
                names: ImportNames::default(),
                line: line_no,
            });
            resolved_path
        } else {
            None
        };

        let module_paths = self.module_paths(module);
        for (name, local_name) in parse_names(imported) {
            if name == "*" {
                continue;
            }

            let mut resolved_path = None;
            for module_path in &module_paths {
                let candidate = module_path.join(name.replace('.', "/"));
                resolved_path = self.resolve_module_path(&candidate);
                if resolved_path.is_some() {
                    break;
                }
            }

            if resolved_path.is_none() {
                resolved_path.clone_from(&parent_resolved);
            }

            let kind = classify_kind(module, resolved_path.is_some());
            let local = normalize_identifier(&local_name);

            let mut names = ImportNames::new(local.clone().into_iter().collect());
            if let Some(ref local) = local {
                if *local != name {
                    names.aliases.insert(local.clone(), name.clone());
                }
            }

            imports.push(ResolvedImport {
                raw: format!("{module}.{name}"),
                resolved_path,
                kind,
                names,
                line: line_no,
            });
        }
    }

    fn resolve_module(&self, module: &str) -> Option<PathBuf> {
        for module_path in self.module_paths(module) {
            if let Some(path) = self.resolve_module_path(&module_path) {
                return Some(path);
            }
        }
        None
    }

    fn module_paths(&self, module: &str) -> Vec<PathBuf> {
        if module.starts_with('.') {
            return self.relative_module_path(module).into_iter().collect();
        }

        let mut paths = self.absolute_module_paths(module);
        if let Some(fallback) = normalize_rel_path(Path::new(&module.replace('.', "/"))) {
            if paths.iter().all(|existing| existing != &fallback) {
                paths.push(fallback);
            }
        }
        paths
    }

    fn relative_module_path(&self, module: &str) -> Option<PathBuf> {
        let dots = module.chars().take_while(|ch| *ch == '.').count();
        let tail = module[dots..].trim();
        let mut base = self
            .source_file
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();

        for _ in 1..dots {
            base = base.parent()?.to_path_buf();
        }

        if tail.is_empty() {
            return normalize_rel_path(&base);
        }

        normalize_rel_path(&base.join(tail.replace('.', "/")))
    }

    fn absolute_module_paths(&self, module: &str) -> Vec<PathBuf> {
        let mut parts = module.split('.').filter(|part| !part.is_empty());
        let Some(head) = parts.next() else {
            return Vec::new();
        };

        let tail = parts.collect::<Vec<_>>();
        let mut paths = Vec::new();

        if let Some(package_dirs) = self.workspace.package_dirs.get(head) {
            for package_dir in package_dirs {
                let mut candidate = package_dir.clone();
                for part in &tail {
                    candidate.push(part);
                }
                paths.push(candidate);
            }
        }

        if tail.is_empty() {
            if let Some(module_files) = self.workspace.module_files.get(head) {
                for module_file in module_files {
                    paths.push(module_file.with_extension(""));
                }
            }
        }

        paths
    }

    fn resolve_module_path(&self, module_path: &Path) -> Option<PathBuf> {
        let file_candidate = module_path.with_extension("py");
        if let Some(path) = resolve_file(self.root, &file_candidate) {
            return Some(path);
        }

        resolve_file(self.root, &module_path.join("__init__.py"))
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
            if super::ALWAYS_IGNORED_DIRS.contains(&name.as_ref()) {
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
    for raw in setup_py_root_tokens(&source) {
        push_project_root(root, project_root, &raw, &mut roots);
    }

    roots
}

fn setup_py_root_tokens(source: &str) -> Vec<String> {
    SetupPyRootCollector::new(source)
        .map(|collector| collector.collect())
        .unwrap_or_default()
}

struct SetupPyRootCollector<'src> {
    tree: tree_sitter::Tree,
    source_bytes: &'src [u8],
}

impl<'src> SetupPyRootCollector<'src> {
    fn new(source: &'src str) -> Option<Self> {
        let mut parser = Parser::new();
        let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
        parser.set_language(&language).ok()?;
        let tree = parser.parse(source, None)?;
        Some(Self {
            tree,
            source_bytes: source.as_bytes(),
        })
    }

    fn collect(&self) -> Vec<String> {
        let mut roots = Vec::new();

        walk_depth_first(self.tree.root_node(), |node| {
            if node.kind() != "call" {
                return;
            }

            let Some(function_name) = self.call_function_name(node) else {
                return;
            };

            match function_name.as_str() {
                "find_packages" | "find_namespace_packages" => {
                    if let Some(where_path) = self.call_keyword_string_arg(node, "where") {
                        roots.push(where_path);
                    }
                }
                "setup" => {
                    if let Some(package_dir) = self.setup_package_dir_root(node) {
                        roots.push(package_dir);
                    }
                }
                _ => {}
            }
        });

        let mut deduped = Vec::new();
        for root in roots {
            if deduped.iter().all(|existing| existing != &root) {
                deduped.push(root);
            }
        }

        deduped
    }

    fn call_function_name(&self, call: Node<'_>) -> Option<String> {
        let function = call.child_by_field_name("function")?;
        let text = function.utf8_text(self.source_bytes).ok()?.trim();
        let name = text.rsplit('.').next().unwrap_or(text).trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn keyword_argument_value<'tree>(
        &self,
        call: Node<'tree>,
        keyword: &str,
    ) -> Option<Node<'tree>> {
        let arguments = call.child_by_field_name("arguments")?;
        let mut cursor = arguments.walk();

        for argument in arguments.named_children(&mut cursor) {
            if argument.kind() != "keyword_argument" {
                continue;
            }

            let Some(name_node) = argument.child_by_field_name("name") else {
                continue;
            };
            let Ok(name) = name_node.utf8_text(self.source_bytes) else {
                continue;
            };
            if name.trim() != keyword {
                continue;
            }

            return argument.child_by_field_name("value");
        }

        None
    }

    fn call_keyword_string_arg(&self, call: Node<'_>, keyword: &str) -> Option<String> {
        let value_node = self.keyword_argument_value(call, keyword)?;
        let value = self.python_string_literal(value_node)?;
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn setup_package_dir_root(&self, call: Node<'_>) -> Option<String> {
        let value_node = self.keyword_argument_value(call, "package_dir")?;
        self.dictionary_root_value(value_node)
    }

    fn dictionary_root_value(&self, dictionary: Node<'_>) -> Option<String> {
        if dictionary.kind() != "dictionary" {
            return None;
        }

        let mut cursor = dictionary.walk();
        for entry in dictionary.named_children(&mut cursor) {
            if entry.kind() != "pair" {
                continue;
            }

            let Some(key_node) = entry.child_by_field_name("key") else {
                continue;
            };
            let Some(value_node) = entry.child_by_field_name("value") else {
                continue;
            };

            let Some(key) = self.python_string_literal(key_node) else {
                continue;
            };
            if !key.trim().is_empty() {
                continue;
            }

            let value = self.python_string_literal(value_node)?;
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            return Some(value.to_string());
        }

        None
    }

    fn python_string_literal(&self, node: Node<'_>) -> Option<String> {
        let raw = node.utf8_text(self.source_bytes).ok()?.trim();
        if raw.is_empty() {
            return None;
        }

        let quote_start = raw.find(|ch| ch == '\'' || ch == '"')?;
        let quoted = &raw[quote_start..];

        let value = if quoted.starts_with("\"\"\"") && quoted.ends_with("\"\"\"") {
            if quoted.len() < 6 {
                return None;
            }
            &quoted[3..quoted.len() - 3]
        } else if quoted.starts_with("'''") && quoted.ends_with("'''") {
            if quoted.len() < 6 {
                return None;
            }
            &quoted[3..quoted.len() - 3]
        } else {
            let mut chars = quoted.chars();
            let quote = chars.next()?;
            if !matches!(quote, '\'' | '"') {
                return None;
            }
            if quoted.len() < 2 || !quoted.ends_with(quote) {
                return None;
            }
            &quoted[1..quoted.len() - 1]
        };

        Some(value.to_string())
    }
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
