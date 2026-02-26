use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Node, Parser};
use walkdir::WalkDir;

use crate::lang::CodeLanguage;
use crate::symbols::{raw_node_text, walk_depth_first};

use super::{
    ALWAYS_IGNORED_DIRS, ImportKind, ImportNames, LanguageResolver, ReexportBinding,
    ResolvedImport, WorkspacePackages, normalize_identifier, normalize_rel_path, path_is_ignored,
    resolve_file, resolve_with_exts, sanitize_specifier, slash_path, to_root_relative,
};

// ---------------------------------------------------
// src/resolve/tsjs.rs
//
// const TSJS_EXTS                                 L71
// pub(crate) struct TsjsResolver                  L77
// enum ImportPattern                              L84
// struct ImportRequest                            L93
//   pub(super) fn new()                          L100
//   pub(super) fn resolve()                      L130
//   fn classify_local_kind()                     L165
//   fn resolve_workspace_specifier()             L177
//   fn collect_requests()                        L181
//   fn parse_tree()                              L203
//   fn resolve()                                 L226
//   fn classify()                                L233
//   fn parse()                                   L268
//   fn source_field()                            L312
//   fn require_arg()                             L320
//   fn bindings_before_source()                  L327
//   fn declarator_name()                         L347
//   fn first_named_child_of_kind()               L364
//   fn string_literal_value()                    L370
// struct ImportBindings                          L397
//   fn from_import()                             L404
//   fn from_require()                            L421
//   fn from_braced()                             L436
//   fn parse_segment()                           L473
//   fn dedupe()                                  L497
// pub(crate) fn collect_specifiers()             L509
// pub(super) fn discover_workspace_packages()    L522
// struct WorkspacePatternSet                     L600
//   fn discover()                                L607
//   fn compile_include()                         L633
//   fn compile_exclude()                         L637
//   fn path_allowed()                            L641
//   fn read_pnpm_patterns()                      L660
//   fn read_package_json_patterns()              L706
//   fn compile_globset()                         L743
//   fn globset_matches()                         L759
// struct PackageManifest                         L776
//   fn read_name()                               L779
//   fn read_json()                               L789
// struct WorkspaceMatch                          L800
//   fn find()                                    L807
//   fn resolve()                                 L817
//   fn split_specifier()                         L869
//   fn resolve_target()                          L899
//   fn export_target()                           L910
//   fn first_export_string()                     L940
// ---------------------------------------------------

const TSJS_EXTS: &[&str] = &[
    "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "d.ts", "d.mts", "d.cts",
];

/// Resolves TypeScript/JavaScript import and require statements against the
/// workspace packages and tsconfig paths.
pub(crate) struct TsjsResolver<'a> {
    root: &'a Path,
    resolver: Resolver,
    workspace_packages: &'a WorkspacePackages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportPattern {
    ImportFrom,
    SideEffect,
    ExportFrom,
    RequireAssign,
    RequireCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportRequest {
    raw: String,
    names: ImportNames,
    line: usize,
}

impl<'a> TsjsResolver<'a> {
    pub(super) fn new(root: &'a Path, workspace_packages: &'a WorkspacePackages) -> Self {
        let mut options = ResolveOptions::default();
        options.tsconfig = Some(TsconfigDiscovery::Auto);
        options.condition_names = vec![
            "source".to_string(),
            "import".to_string(),
            "node".to_string(),
            "default".to_string(),
        ];
        options.extensions = vec![
            ".ts".to_string(),
            ".tsx".to_string(),
            ".mts".to_string(),
            ".cts".to_string(),
            ".js".to_string(),
            ".jsx".to_string(),
            ".mjs".to_string(),
            ".cjs".to_string(),
            ".json".to_string(),
            ".node".to_string(),
        ];
        options.main_files = vec!["index".to_string()];

        Self {
            root,
            resolver: Resolver::new(options),
            workspace_packages,
        }
    }

    pub(super) fn resolve(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
        let source_abs = self.root.join(source_file);
        let mut imports = Vec::new();

        for request in Self::collect_requests(source_file, source) {
            let Some(specifier) = sanitize_specifier(&request.raw) else {
                continue;
            };

            let mut resolved_path = self
                .resolver
                .resolve_file(&source_abs, &specifier)
                .ok()
                .and_then(|resolution| to_root_relative(self.root, resolution.path()));
            let kind = if resolved_path.is_some() {
                self.classify_local_kind(&specifier)
            } else if let Some(path) = self.resolve_workspace_specifier(&specifier) {
                resolved_path = Some(path);
                ImportKind::Workspace
            } else {
                ImportKind::External
            };

            imports.push(ResolvedImport {
                raw: request.raw,
                resolved_path,
                kind,
                names: request.names,
                line: request.line,
            });
        }

        imports
    }

    fn classify_local_kind(&self, specifier: &str) -> ImportKind {
        if specifier.starts_with('.') || specifier.starts_with('/') {
            return ImportKind::Relative;
        }

        if WorkspaceMatch::find(specifier, self.workspace_packages).is_some() {
            return ImportKind::Workspace;
        }

        ImportKind::TsconfigPath
    }

    fn resolve_workspace_specifier(&self, specifier: &str) -> Option<PathBuf> {
        WorkspaceMatch::find(specifier, self.workspace_packages)?.resolve(self.root)
    }

    fn collect_requests(source_file: &Path, source: &str) -> Vec<ImportRequest> {
        let Some(tree) = Self::parse_tree(source_file, source) else {
            return Vec::new();
        };

        let source_bytes = source.as_bytes();
        let mut requests = Vec::new();

        walk_depth_first(tree.root_node(), |node| {
            let Some(pattern) = ImportPattern::classify(node, source_bytes) else {
                return;
            };
            if let Some(request) = pattern.parse(node, source_bytes) {
                requests.push(request);
            }
        });

        let mut seen = HashSet::new();
        requests.retain(|request| seen.insert((request.raw.clone(), request.line)));
        requests
    }

    fn parse_tree(source_file: &Path, source: &str) -> Option<tree_sitter::Tree> {
        let language = CodeLanguage::from_path(source_file).filter(|l| {
            matches!(
                l,
                CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx
            )
        })?;

        let ts_language: Language = match language {
            CodeLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            CodeLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            CodeLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        };

        let mut parser = Parser::new();
        parser.set_language(&ts_language).ok()?;
        parser.parse(source, None)
    }
}

impl LanguageResolver for TsjsResolver<'_> {
    /// Resolve all TypeScript/JavaScript imports in the given source file.
    fn resolve(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
        self.resolve(source_file, source)
    }
}

impl ImportPattern {
    /// Classify a tree-sitter node as an import pattern, if applicable.
    fn classify(node: Node<'_>, source: &[u8]) -> Option<Self> {
        match node.kind() {
            "import_statement" => {
                Self::source_field(node, source)?;
                if Self::bindings_before_source(node, source).is_some() {
                    Some(Self::ImportFrom)
                } else {
                    Some(Self::SideEffect)
                }
            }
            "export_statement" => {
                node.child_by_field_name("source")
                    .and_then(|n| Self::string_literal_value(n, source))?;
                Some(Self::ExportFrom)
            }
            "call_expression" => {
                let function_node = node.child_by_field_name("function")?;
                if function_node.kind() != "identifier" {
                    return None;
                }
                if raw_node_text(function_node, source) != Some("require") {
                    return None;
                }
                Self::require_arg(node, source)?;
                if Self::declarator_name(node, source).is_some() {
                    Some(Self::RequireAssign)
                } else {
                    Some(Self::RequireCall)
                }
            }
            _ => None,
        }
    }

    /// Extract an `ImportRequest` from the already-classified node.
    fn parse(&self, node: Node<'_>, source: &[u8]) -> Option<ImportRequest> {
        let line = node.start_position().row + 1;

        match self {
            Self::ImportFrom => {
                let raw = Self::source_field(node, source)?;
                let bindings = Self::bindings_before_source(node, source)?;
                Some(ImportRequest {
                    raw,
                    names: ImportBindings::from_import(&bindings),
                    line,
                })
            }
            Self::SideEffect => Some(ImportRequest {
                raw: Self::source_field(node, source)?,
                names: ImportNames::default(),
                line,
            }),
            Self::ExportFrom => Some(ImportRequest {
                raw: node
                    .child_by_field_name("source")
                    .and_then(|n| Self::string_literal_value(n, source))?,
                names: ImportNames::default(),
                line,
            }),
            Self::RequireAssign => {
                let raw = Self::require_arg(node, source)?;
                let bindings = Self::declarator_name(node, source)?;
                Some(ImportRequest {
                    raw,
                    names: ImportBindings::from_require(&bindings),
                    line,
                })
            }
            Self::RequireCall => Some(ImportRequest {
                raw: Self::require_arg(node, source)?,
                names: ImportNames::default(),
                line,
            }),
        }
    }

    /// Extract the module specifier from an import statement's `source` field,
    /// falling back to the first string child for bare `import 'x'` forms.
    fn source_field(node: Node<'_>, source: &[u8]) -> Option<String> {
        let source_node = node
            .child_by_field_name("source")
            .or_else(|| Self::first_named_child_of_kind(node, "string"))?;
        Self::string_literal_value(source_node, source)
    }

    /// Extract the first string argument from a `require(...)` call.
    fn require_arg(node: Node<'_>, source: &[u8]) -> Option<String> {
        let arguments = node.child_by_field_name("arguments")?;
        let first_argument = arguments.named_child(0)?;
        Self::string_literal_value(first_argument, source)
    }

    /// Extract binding text from an import statement (everything before the source string).
    fn bindings_before_source(node: Node<'_>, source: &[u8]) -> Option<String> {
        let source_node = node
            .child_by_field_name("source")
            .or_else(|| Self::first_named_child_of_kind(node, "string"))?;

        let mut binding_node = None;
        for index in 0..node.named_child_count() {
            let Some(child) = node.named_child(index as u32) else {
                continue;
            };
            if child.start_byte() >= source_node.start_byte() {
                break;
            }
            binding_node = Some(child);
        }

        binding_node.and_then(|child| raw_node_text(child, source).map(str::to_string))
    }

    /// Extract the binding name from a `const X = require(...)` variable declarator.
    fn declarator_name(call_expression: Node<'_>, source: &[u8]) -> Option<String> {
        let parent = call_expression.parent()?;
        if parent.kind() != "variable_declarator" {
            return None;
        }

        let value_node = parent.child_by_field_name("value")?;
        if value_node.start_byte() != call_expression.start_byte()
            || value_node.end_byte() != call_expression.end_byte()
        {
            return None;
        }

        let binding_node = parent.child_by_field_name("name")?;
        raw_node_text(binding_node, source).map(str::to_string)
    }

    fn first_named_child_of_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        (0..node.named_child_count())
            .filter_map(|index| node.named_child(index as u32))
            .find(|child| child.kind() == kind)
    }

    fn string_literal_value(node: Node<'_>, source: &[u8]) -> Option<String> {
        let raw = node.utf8_text(source).ok()?.trim();
        if raw.is_empty() || raw.contains("${") {
            return None;
        }

        let value = raw
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
            .or_else(|| {
                raw.strip_prefix('\'')
                    .and_then(|inner| inner.strip_suffix('\''))
            })
            .or_else(|| {
                raw.strip_prefix('`')
                    .and_then(|inner| inner.strip_suffix('`'))
            })
            .unwrap_or(raw)
            .trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
}

struct ImportBindings;

impl ImportBindings {
    /// Parse ES import bindings: `import X, { Y as Z } from '...'`
    ///
    /// Handles default imports, namespace imports (`* as X`), named imports
    /// (`{ X, Y as Z }`), and combinations (`X, { Y }`). Strips `type` prefixes.
    fn from_import(raw: &str) -> ImportNames {
        let mut result = ImportNames::default();
        let binding = raw.trim().trim_start_matches("type ").trim();

        if let Some((default_part, rest)) = binding.split_once(',') {
            if let Some(name) = normalize_identifier(default_part) {
                result.locals.push(name);
            }
            let segment = Self::parse_segment(rest);
            result.locals.extend(segment.locals);
            result.aliases.extend(segment.aliases);
        } else {
            let segment = Self::parse_segment(binding);
            result.locals.extend(segment.locals);
            result.aliases.extend(segment.aliases);
        }

        result.locals = Self::dedupe(result.locals);
        result
    }

    /// Parse require bindings: `const X = require(...)` or `const { X } = require(...)`
    fn from_require(raw: &str) -> ImportNames {
        let binding = raw.trim();
        if binding.starts_with('{') {
            return Self::from_braced(binding);
        }

        ImportNames::new(
            normalize_identifier(binding)
                .map(|name| vec![name])
                .unwrap_or_default(),
        )
    }

    /// Parse `{ foo as bar, type Baz }` or `{ foo: bar, baz = 1 }`.
    ///
    /// Handles ES `as` aliases, require `:` renames, default values (`= ...`),
    /// and `type` prefixes.
    fn from_braced(raw: &str) -> ImportNames {
        let start = raw.find('{');
        let end = raw.rfind('}');
        let Some((start, end)) = start.zip(end) else {
            return ImportNames::default();
        };
        if end <= start {
            return ImportNames::default();
        }

        let inner = &raw[start + 1..end];
        let mut result = ImportNames::default();

        for item in inner.split(',') {
            let token = item.trim().trim_start_matches("type ").trim();
            if token.is_empty() {
                continue;
            }

            let (original, local) = if let Some((orig, alias)) = token.split_once(" as ") {
                (Some(orig.trim()), alias.trim())
            } else if let Some((_, value)) = token.split_once(':') {
                let value = value
                    .split_once('=')
                    .map(|(v, _)| v)
                    .unwrap_or(value)
                    .trim();
                (None, value)
            } else {
                let value = token
                    .split_once('=')
                    .map(|(v, _)| v)
                    .unwrap_or(token)
                    .trim();
                (None, value)
            };

            if let Some(local_name) = normalize_identifier(local) {
                result.locals.push(local_name.clone());
                if let Some(orig) = original {
                    if let Some(orig_name) = normalize_identifier(orig) {
                        if orig_name != local_name {
                            result.aliases.insert(local_name, orig_name);
                        }
                    }
                }
            }
        }

        result.locals = Self::dedupe(result.locals);
        result
    }

    /// Parse a single binding segment (braced group, namespace `* as X`, or bare identifier).
    fn parse_segment(raw: &str) -> ImportNames {
        let segment = raw.trim();
        if segment.is_empty() {
            return ImportNames::default();
        }

        if segment.starts_with('{') {
            return Self::from_braced(segment);
        }

        if let Some(alias) = segment
            .strip_prefix('*')
            .map(str::trim)
            .and_then(|value| value.strip_prefix("as "))
            .and_then(normalize_identifier)
        {
            let mut names = ImportNames::new(vec![alias]);
            names.is_namespace = true;
            return names;
        }

        ImportNames::new(
            normalize_identifier(segment)
                .map(|value| vec![value])
                .unwrap_or_default(),
        )
    }

    fn dedupe(names: Vec<String>) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for name in names {
            if seen.insert(name.clone()) {
                deduped.push(name);
            }
        }
        deduped
    }
}

/// Collect re-exported symbol bindings from `export { ... } from '...'` forms.
pub(crate) fn collect_reexports(source_file: &Path, source: &str) -> Vec<ReexportBinding> {
    let Some(tree) = TsjsResolver::parse_tree(source_file, source) else {
        return Vec::new();
    };

    let source_bytes = source.as_bytes();
    let mut bindings = Vec::new();
    walk_depth_first(tree.root_node(), |node| {
        if node.kind() != "export_statement" {
            return;
        }

        let Some(raw_specifier) = node
            .child_by_field_name("source")
            .and_then(|value| ImportPattern::string_literal_value(value, source_bytes))
        else {
            return;
        };

        let line = node.start_position().row as usize + 1;
        walk_depth_first(node, |child| {
            if child.kind() != "export_specifier" {
                return;
            }

            let Some((definition_name, exported_name)) =
                export_specifier_names(child, source_bytes)
            else {
                return;
            };

            bindings.push(ReexportBinding {
                raw_specifier: raw_specifier.clone(),
                exported_name,
                definition_name,
                line,
            });
        });
    });

    let mut seen = HashSet::new();
    bindings.retain(|binding| {
        seen.insert((
            binding.line,
            binding.raw_specifier.clone(),
            binding.exported_name.clone(),
            binding.definition_name.clone(),
        ))
    });
    bindings
}

fn export_specifier_names(node: Node<'_>, source: &[u8]) -> Option<(String, String)> {
    let raw = node.utf8_text(source).ok()?.trim();
    if raw.is_empty() {
        return None;
    }

    let (definition_raw, exported_raw) = raw
        .split_once(" as ")
        .map(|(left, right)| (left.trim(), right.trim()))
        .unwrap_or((raw, raw));
    let definition_name = normalize_identifier(definition_raw)?;
    let exported_name = normalize_identifier(exported_raw)?;
    Some((definition_name, exported_name))
}

pub(crate) fn collect_specifiers(source_file: &Path, source: &str) -> Vec<String> {
    let mut specifiers = BTreeSet::new();
    for request in TsjsResolver::collect_requests(source_file, source) {
        if let Some(specifier) = sanitize_specifier(&request.raw) {
            specifiers.insert(specifier);
        }
    }

    specifiers.into_iter().collect()
}

/// Discover workspace packages by walking the project tree for `package.json` files
/// and matching against pnpm/npm/yarn workspace patterns.
pub(super) fn discover_workspace_packages(root: &Path, ignore_set: &GlobSet) -> WorkspacePackages {
    let patterns = WorkspacePatternSet::discover(root);
    let include_set = patterns.compile_include();
    let exclude_set = patterns.compile_exclude();

    let mut packages = WorkspacePackages::new();
    let mut package_json_paths = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            let Some(rel) = to_root_relative(root, entry.path()) else {
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

        if entry.file_name().to_string_lossy() != "package.json" {
            continue;
        }

        let Some(rel) = to_root_relative(root, entry.path()) else {
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
        if !patterns.path_allowed(rel_dir, include_set.as_ref(), exclude_set.as_ref()) {
            continue;
        }

        let abs = root.join(&rel_manifest);
        let Some(package_name) = PackageManifest::read_name(&abs) else {
            continue;
        };

        let Some(package_root) = normalize_rel_path(rel_dir) else {
            continue;
        };
        packages.entry(package_name).or_insert(package_root);
    }

    packages
}

// ---------------------------------------------------------------------------
// WorkspacePatternSet — include/exclude globs from pnpm-workspace.yaml and
// package.json workspace configs.
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct WorkspacePatternSet {
    includes: Vec<String>,
    excludes: Vec<String>,
}

impl WorkspacePatternSet {
    /// Discover workspace patterns from pnpm-workspace.yaml and package.json.
    fn discover(root: &Path) -> Self {
        let mut patterns = Self::default();
        let mut all_patterns = Vec::new();
        all_patterns.extend(Self::read_pnpm_patterns(root));
        all_patterns.extend(Self::read_package_json_patterns(root));

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

    fn compile_include(&self) -> Option<GlobSet> {
        Self::compile_globset(&self.includes)
    }

    fn compile_exclude(&self) -> Option<GlobSet> {
        Self::compile_globset(&self.excludes)
    }

    fn path_allowed(
        &self,
        rel_dir: &Path,
        include_set: Option<&GlobSet>,
        exclude_set: Option<&GlobSet>,
    ) -> bool {
        let slash = slash_path(rel_dir);

        if include_set.is_some_and(|set| !Self::globset_matches(set, &slash)) {
            return false;
        }

        if exclude_set.is_some_and(|set| Self::globset_matches(set, &slash)) {
            return false;
        }

        true
    }

    fn read_pnpm_patterns(root: &Path) -> Vec<String> {
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

    fn read_package_json_patterns(root: &Path) -> Vec<String> {
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

    fn globset_matches(set: &GlobSet, slash_path: &str) -> bool {
        if set.is_match(slash_path) {
            return true;
        }

        if slash_path.is_empty() {
            return set.is_match("./");
        }

        set.is_match(format!("{slash_path}/"))
    }
}

// ---------------------------------------------------------------------------
// PackageManifest — helpers for reading package.json fields.
// ---------------------------------------------------------------------------

struct PackageManifest;

impl PackageManifest {
    fn read_name(abs_path: &Path) -> Option<String> {
        let value = Self::read_json(abs_path)?;
        let name = value.get("name")?.as_str()?.trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn read_json(abs_path: &Path) -> Option<Value> {
        let raw = fs::read_to_string(abs_path).ok()?;
        serde_json::from_str::<Value>(&raw).ok()
    }
}

// ---------------------------------------------------------------------------
// WorkspaceMatch — match and resolve a specifier against workspace packages.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct WorkspaceMatch {
    package_root: PathBuf,
    subpath: Option<String>,
}

impl WorkspaceMatch {
    /// Match a specifier like `@myorg/lib/utils` against known workspace packages.
    fn find(specifier: &str, workspace_packages: &WorkspacePackages) -> Option<Self> {
        let (package_name, subpath) = Self::split_specifier(specifier)?;
        let package_root = workspace_packages.get(&package_name)?.clone();
        Some(Self {
            package_root,
            subpath,
        })
    }

    /// Resolve this workspace match to a file path within the project.
    fn resolve(&self, root: &Path) -> Option<PathBuf> {
        let manifest =
            PackageManifest::read_json(&root.join(&self.package_root).join("package.json"));

        if let Some(subpath) = &self.subpath {
            if let Some(target) = manifest
                .as_ref()
                .and_then(|value| value.get("exports"))
                .and_then(|exports| Self::export_target(exports, &format!("./{subpath}")))
                .and_then(|target| Self::resolve_target(root, &self.package_root, &target))
            {
                return Some(target);
            }

            let candidate = self.package_root.join(subpath);
            return resolve_with_exts(root, &candidate, TSJS_EXTS)
                .or_else(|| resolve_file(root, &candidate));
        }

        if let Some(target) = manifest
            .as_ref()
            .and_then(|value| value.get("exports"))
            .and_then(|exports| Self::export_target(exports, "."))
            .and_then(|target| Self::resolve_target(root, &self.package_root, &target))
        {
            return Some(target);
        }

        if let Some(value) = manifest.as_ref() {
            for field in ["types", "typings", "module", "main"] {
                if let Some(target) = value
                    .get(field)
                    .and_then(Value::as_str)
                    .and_then(|target| Self::resolve_target(root, &self.package_root, target))
                {
                    return Some(target);
                }
            }
        }

        for base in [
            self.package_root.join("src/index"),
            self.package_root.join("index"),
        ] {
            if let Some(path) = resolve_with_exts(root, &base, TSJS_EXTS) {
                return Some(path);
            }
        }

        None
    }

    fn split_specifier(specifier: &str) -> Option<(String, Option<String>)> {
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

    fn resolve_target(root: &Path, package_root: &Path, raw_target: &str) -> Option<PathBuf> {
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
            Value::Array(values) => values
                .iter()
                .find_map(|value| Self::export_target(value, key)),
            Value::Object(table) => {
                if let Some(value) = table.get(key) {
                    return Self::first_export_string(value);
                }

                if key == "." {
                    let has_subpath_keys = table.keys().any(|entry| entry.starts_with('.'));
                    if !has_subpath_keys {
                        return Self::first_export_string(exports);
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
            Value::Array(values) => values.iter().find_map(Self::first_export_string),
            Value::Object(table) => {
                for key in ["types", "import", "module", "default", "require"] {
                    if let Some(value) = table.get(key).and_then(Self::first_export_string) {
                        return Some(value);
                    }
                }
                table.values().find_map(Self::first_export_string)
            }
            _ => None,
        }
    }
}
