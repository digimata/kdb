use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    ImportKind, ImportNames, LanguageResolver, ResolvedImport, list_go_package_files,
    normalize_identifier, normalize_rel_path, to_root_relative,
};

// -------------------------------------
// kdb/src/resolve/go.rs
//
// enum GoWorkBlock                  L40
// struct ParsedGoWork               L46
//   fn parse()                      L54
//   fn parse_use_path()            L116
//   fn parse_replace_entry()       L122
// pub struct GoWorkspaceCache      L141
//   pub(super) fn build()          L148
// pub(crate) struct GoResolver     L176
//   pub(super) fn new()            L183
//   fn resolve_source()            L189
//   fn push_import()               L229
//   fn resolve_import()            L265
//   fn workspace_module_match()    L287
//   fn resolve()                   L314
// fn go_module_name()              L320
// fn parse_import_line()           L339
// fn import_names()                L367
// fn classify_kind()               L392
// fn directive_body()              L408
// fn strip_line_comment()          L422
// fn parse_local_dir()             L427
// fn is_local_path()               L442
// fn trim_go_token()               L454
// fn push_unique_path()            L459
// -------------------------------------

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

impl ParsedGoWork {
    /// Parse the `go.work` file at `root`, extracting `use` and `replace`
    /// directives into their respective collections.
    fn parse(root: &Path) -> Self {
        let source = match fs::read_to_string(root.join("go.work")) {
            Ok(source) => source,
            Err(_) => return Self::default(),
        };

        let mut parsed = Self::default();
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
                    if let Some(path) = Self::parse_use_path(root, Path::new(""), line) {
                        push_unique_path(&mut parsed.use_dirs, path);
                    }
                    continue;
                }
                Some(GoWorkBlock::Replace) => {
                    if let Some((module_path, local_dir)) =
                        Self::parse_replace_entry(root, Path::new(""), line)
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
                } else if let Some(path) = Self::parse_use_path(root, Path::new(""), body) {
                    push_unique_path(&mut parsed.use_dirs, path);
                }
                continue;
            }

            if let Some(body) = directive_body(line, "replace") {
                if body == "(" {
                    block = Some(GoWorkBlock::Replace);
                } else if let Some((module_path, local_dir)) =
                    Self::parse_replace_entry(root, Path::new(""), body)
                {
                    parsed.replace_dirs.insert(module_path, local_dir);
                }
            }
        }

        parsed
    }

    /// Extract a local directory path from a `use` directive value.
    fn parse_use_path(root: &Path, base_dir: &Path, value: &str) -> Option<PathBuf> {
        let token = value.split_whitespace().next()?;
        parse_local_dir(root, base_dir, token)
    }

    /// Parse a `replace` directive line into `(module_path, local_dir)`.
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
}

/// Cached Go workspace data: maps module import paths to their local directories.
#[derive(Debug, Clone, Default)]
pub struct GoWorkspaceCache {
    pub modules_by_path: HashMap<String, PathBuf>,
}

impl GoWorkspaceCache {
    /// Build the workspace cache by parsing `go.work` and discovering module
    /// names from `go.mod` files in each workspace directory.
    pub(super) fn build(root: &Path) -> Self {
        let parsed = ParsedGoWork::parse(root);
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
}

/// Resolves Go import statements against the workspace cache.
pub(crate) struct GoResolver<'a> {
    root: &'a Path,
    workspace: &'a GoWorkspaceCache,
}

impl<'a> GoResolver<'a> {
    /// Create a new resolver for the given project root and workspace cache.
    pub(super) fn new(root: &'a Path, workspace: &'a GoWorkspaceCache) -> Self {
        Self { root, workspace }
    }

    /// Walk source lines, extract import statements, and resolve each against
    /// the workspace module map.
    fn resolve_source(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
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
                    self.push_import(source_file, line_no, alias, &spec, &mut imports);
                }
                continue;
            }

            if no_comment.starts_with("import (") {
                in_block = true;
                continue;
            }

            if let Some(rest) = no_comment.strip_prefix("import ") {
                if let Some((alias, spec)) = parse_import_line(rest) {
                    self.push_import(source_file, line_no, alias, &spec, &mut imports);
                }
            }
        }

        imports
    }

    /// Resolve a single import specifier and push one or more `ResolvedImport`
    /// entries (one per resolved file for package-level imports).
    fn push_import(
        &self,
        source_file: &Path,
        line: usize,
        alias: Option<String>,
        spec: &str,
        imports: &mut Vec<ResolvedImport>,
    ) {
        let resolved_paths = self.resolve_import(source_file, spec);
        let kind = classify_kind(spec, !resolved_paths.is_empty());
        let names = import_names(alias.as_deref(), spec);

        if resolved_paths.is_empty() {
            imports.push(ResolvedImport {
                raw: spec.to_string(),
                resolved_path: None,
                kind,
                names: names.clone(),
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

    /// Map an import specifier to file paths: relative imports resolve against
    /// the source directory; absolute imports match against workspace modules.
    fn resolve_import(&self, source_file: &Path, spec: &str) -> Vec<PathBuf> {
        if spec.starts_with("./") || spec.starts_with("../") {
            let dir = source_file.parent().unwrap_or(Path::new("")).join(spec);
            return list_go_package_files(self.root, &dir);
        }

        let Some((module_path, module_dir)) = self.workspace_module_match(spec) else {
            return Vec::new();
        };

        if spec == module_path {
            return list_go_package_files(self.root, module_dir);
        }

        let Some(rest) = spec.strip_prefix(module_path) else {
            return Vec::new();
        };

        list_go_package_files(self.root, &module_dir.join(rest.trim_start_matches('/')))
    }

    /// Find the workspace module whose path is the longest prefix of `spec`.
    fn workspace_module_match(&self, spec: &str) -> Option<(&str, &PathBuf)> {
        let mut best_match: Option<(&str, &PathBuf)> = None;

        for (module_path, module_dir) in &self.workspace.modules_by_path {
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
}

impl LanguageResolver for GoResolver<'_> {
    /// Resolve all Go imports in the given source file.
    fn resolve(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
        self.resolve_source(source_file, source)
    }
}

/// Read the `module` directive from a `go.mod` file inside `module_dir`.
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

/// Parse a single Go import line, returning `(alias, specifier)`.
///
/// The specifier is the quoted string; the alias is the optional identifier
/// before the quoted string (e.g. `alias "path/to/pkg"`).
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

/// Derive the local binding name from an import alias or the last path segment.
///
/// When an explicit alias differs from the package's default name (last path
/// segment), records the mapping in `ImportNames.aliases`.
fn import_names(alias: Option<&str>, spec: &str) -> ImportNames {
    let default_name = spec.rsplit('/').next().and_then(normalize_identifier);

    if let Some(alias) = alias {
        if alias == "." {
            let mut names = ImportNames::new(default_name.into_iter().collect());
            names.is_namespace = true;
            return names;
        }
        if alias != "_" {
            let local = normalize_identifier(alias);
            let mut result = ImportNames::new(local.clone().into_iter().collect());
            if let (Some(local), Some(def)) = (&local, &default_name) {
                if local != def {
                    result.aliases.insert(local.clone(), def.clone());
                }
            }
            return result;
        }
    }

    ImportNames::new(default_name.into_iter().collect())
}

/// Classify a Go import specifier as relative, workspace, or external.
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

/// Extract the body of a Go file directive (e.g. `module`, `use`, `replace`).
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

/// Strip a Go line comment (`//`) and trim whitespace.
fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line).trim()
}

/// Resolve a raw token to a local directory path relative to `root`.
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

/// Return `true` if the value looks like a local file-system path.
fn is_local_path(value: &str) -> bool {
    if value == "." || value == ".." {
        return true;
    }
    if value.starts_with("./") || value.starts_with("../") {
        return true;
    }

    Path::new(value).is_absolute()
}

/// Strip surrounding quotes (double or backtick) from a Go token.
fn trim_go_token(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('`')
}

/// Push a path into `paths` only if it is not already present.
fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if paths.iter().all(|existing| existing != &candidate) {
        paths.push(candidate);
    }
}
