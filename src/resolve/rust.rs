use globset::GlobSet;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value as TomlValue;
use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

use crate::lang::CodeLanguage;
use crate::symbols::walk_depth_first;

use super::{
    ImportKind, ImportNames, LanguageResolver, ReexportBinding, ResolvedImport,
    exported_symbol_names, normalize_identifier, resolve_file,
};

// --------------------------------------------
// projects/kdb/src/resolve/rust.rs
//
// enum SourceImport                        L78
// struct RustImportCollector               L83
//   fn new()                               L89
//   fn collect()                           L96
//   fn parse_tree()                       L121
//   fn parse_mod_item()                   L128
//   fn parse_use_declaration()            L155
//   fn parse_use_path()                   L167
//   fn use_path_from_text()               L179
//   fn first_named_child_of_kind()        L194
// fn collect_source_imports()             L202
// pub(crate) fn collect_mod_and_use()     L207
// pub(crate) fn collect_reexports()       L224
// fn is_public_use_declaration()          L268
// struct ParsedManifest                   L283
//   fn parse()                            L292
//   fn src_root()                         L333
// struct LocalDependency                  L342
// pub struct RustWorkspaceCrate           L352
// pub struct RustWorkspaceCache           L362
//   pub(super) fn build()                 L369
// pub(crate) struct RustResolver          L440
//   pub(super) fn new()                   L447
//   fn resolve_source()                   L453
//   fn resolve()                          L526
// struct CrateContext                     L534
//   fn from_workspace()                   L545
//   fn resolve_mod_decl()                 L582
//   fn resolve_use()                      L604
// fn discover_manifest_paths()            L681
// fn parse_crate_root_files()             L728
// fn push_manifest_entry_path()           L753
// fn default_crate_root_files()           L763
// fn normalize_manifest_path()            L772
// fn collect_dependency_sections()        L782
// fn parse_local_dependency()             L815
// fn resolve_dependency_root()            L871
// fn crate_root_for_name()                L896
// fn crate_import_name()                  L910
// fn parse_use_prefix()                   L915
// fn parse_use_head()                     L927
// fn single_group_item_path()             L939
// fn find_crate_root()                    L964
// fn classify_use_kind()                  L984
// fn rust_module_path()                  L1007
// fn rust_crate_entry_path()             L1025
// fn rust_file_candidates()              L1042
// fn looks_like_module_segment()         L1051
// fn source_segments()                   L1060
// fn imported_names()                    L1084
// fn split_brace_group()                 L1139
// fn split_brace_items()                 L1154
// fn expand_brace_imports()              L1184
// fn last_segment()                      L1221
// fn dedupe_names()                      L1238
// --------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SourceImport {
    Mod { name: String, line: usize },
    Use { path: String, line: usize },
}

struct RustImportCollector<'src> {
    source: &'src str,
    source_bytes: &'src [u8],
}

impl<'src> RustImportCollector<'src> {
    fn new(source: &'src str) -> Self {
        Self {
            source,
            source_bytes: source.as_bytes(),
        }
    }

    fn collect(&self) -> Vec<SourceImport> {
        let Some(tree) = self.parse_tree() else {
            return Vec::new();
        };

        let mut imports = Vec::new();
        walk_depth_first(tree.root_node(), |node| match node.kind() {
            "mod_item" => {
                if let Some(item) = self.parse_mod_item(node) {
                    imports.push(item);
                }
            }
            "use_declaration" => {
                if let Some(item) = self.parse_use_declaration(node) {
                    imports.push(item);
                }
            }
            _ => {}
        });

        let mut seen = HashSet::new();
        imports.retain(|item| seen.insert(item.clone()));
        imports
    }

    fn parse_tree(&self) -> Option<tree_sitter::Tree> {
        let mut parser = Parser::new();
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&language).ok()?;
        parser.parse(self.source, None)
    }

    fn parse_mod_item(&self, node: Node<'_>) -> Option<SourceImport> {
        if node.child_by_field_name("body").is_some() {
            return None;
        }

        if !node
            .utf8_text(self.source_bytes)
            .ok()
            .is_some_and(|text| text.trim_end().ends_with(';'))
        {
            return None;
        }

        let name_node = node
            .child_by_field_name("name")
            .or_else(|| Self::first_named_child_of_kind(node, "identifier"))?;
        let name = name_node.utf8_text(self.source_bytes).ok()?.trim();
        if name.is_empty() {
            return None;
        }

        Some(SourceImport::Mod {
            name: name.to_string(),
            line: node.start_position().row as usize + 1,
        })
    }

    fn parse_use_declaration(&self, node: Node<'_>) -> Option<SourceImport> {
        let path = self.parse_use_path(node)?;
        if path.is_empty() {
            return None;
        }

        Some(SourceImport::Use {
            path,
            line: node.start_position().row as usize + 1,
        })
    }

    fn parse_use_path(&self, node: Node<'_>) -> Option<String> {
        node.child_by_field_name("argument")
            .and_then(|argument| {
                argument
                    .utf8_text(self.source_bytes)
                    .ok()
                    .map(str::trim)
                    .map(ToString::to_string)
            })
            .or_else(|| self.use_path_from_text(node))
    }

    fn use_path_from_text(&self, node: Node<'_>) -> Option<String> {
        let text = node.utf8_text(self.source_bytes).ok()?.trim();
        let text = text.trim_end_matches(';').trim();
        let text = text
            .strip_prefix("use ")
            .or_else(|| text.rsplit_once(" use ").map(|(_, rest)| rest))
            .unwrap_or(text)
            .trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }

    fn first_named_child_of_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        (0..node.named_child_count())
            .filter_map(|index| node.named_child(index as u32))
            .find(|child| child.kind() == kind)
    }
}

/// Collect `mod` and `use` items from Rust source using tree-sitter.
fn collect_source_imports(source: &str) -> Vec<SourceImport> {
    RustImportCollector::new(source).collect()
}

/// Return `(mod_names, use_paths)` extracted from Rust source.
pub(crate) fn collect_mod_and_use(source: &str) -> (Vec<String>, Vec<String>) {
    let mut mods = Vec::new();
    let mut uses = Vec::new();

    for item in collect_source_imports(source) {
        match item {
            SourceImport::Mod { name, .. } => mods.push(name),
            SourceImport::Use { path, .. } => uses.push(path),
        }
    }

    (mods, uses)
}

/// Collect public `use` re-export bindings (`pub use ...`) from Rust source.
///
/// The `tree` must have been parsed from `source`.
pub(crate) fn collect_reexports(source: &str, tree: &tree_sitter::Tree) -> Vec<ReexportBinding> {
    let collector = RustImportCollector::new(source);

    let mut bindings = Vec::new();
    walk_depth_first(tree.root_node(), |node| {
        if node.kind() != "use_declaration"
            || !is_public_use_declaration(node, collector.source_bytes)
        {
            return;
        }

        let Some(path) = collector.parse_use_path(node) else {
            return;
        };
        let line = node.start_position().row as usize + 1;
        let ImportNames {
            locals, aliases, ..
        } = imported_names(&path);
        for exported_name in locals {
            let definition_name = aliases
                .get(&exported_name)
                .cloned()
                .unwrap_or_else(|| exported_name.clone());
            bindings.push(ReexportBinding {
                raw_specifier: path.clone(),
                exported_name,
                definition_name,
                line,
            });
        }
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

fn is_public_use_declaration(node: Node<'_>, source: &[u8]) -> bool {
    let Ok(text) = node.utf8_text(source) else {
        return false;
    };
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix("pub") else {
        return false;
    };
    rest.chars()
        .next()
        .is_some_and(|ch| ch.is_whitespace() || ch == '(')
}

/// Parsed contents of a `Cargo.toml` manifest.
#[derive(Debug, Clone, Default)]
struct ParsedManifest {
    package_name: Option<String>,
    crate_root_files: Vec<PathBuf>,
    dependencies: HashMap<String, LocalDependency>,
}

impl ParsedManifest {
    /// Parse a `Cargo.toml` at `rel_manifest` within `root`, using `crate_root`
    /// as the base directory for resolving relative paths in `[lib]`/`[[bin]]`.
    fn parse(root: &Path, rel_manifest: &Path, crate_root: &Path) -> Option<Self> {
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

    /// Derive the source root directory from the first crate root file,
    /// falling back to `<crate_root>/src`.
    fn src_root(&self, crate_root: &Path) -> PathBuf {
        self.crate_root_files
            .first()
            .and_then(|entry| entry.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| crate_root.join("src"))
    }
}

#[derive(Debug, Clone)]
struct LocalDependency {
    alias: String,
    package: Option<String>,
    path: Option<PathBuf>,
    workspace: bool,
}

/// A single Rust crate in the workspace: its name, source root, entry files,
/// and maps from dependency import names to their source roots and entry files.
#[derive(Debug, Clone, Default)]
pub struct RustWorkspaceCrate {
    pub name: String,
    pub src_root: PathBuf,
    pub crate_root_files: Vec<PathBuf>,
    pub dependency_src_roots: HashMap<String, PathBuf>,
    pub dependency_entry_files: HashMap<String, Vec<PathBuf>>,
}

/// Cached Rust workspace data: maps crate roots to their parsed metadata.
#[derive(Debug, Clone, Default)]
pub struct RustWorkspaceCache {
    pub crates_by_root: HashMap<PathBuf, RustWorkspaceCrate>,
}

impl RustWorkspaceCache {
    /// Build the workspace cache by discovering all `Cargo.toml` manifests,
    /// parsing them, and resolving inter-crate dependency paths.
    pub(super) fn build(root: &Path, ignore_set: &GlobSet) -> Self {
        let manifests = discover_manifest_paths(root, ignore_set);
        let mut manifests_by_root = HashMap::new();
        let mut crate_roots_by_name = HashMap::new();

        for rel_manifest in manifests {
            let crate_root = rel_manifest.parent().unwrap_or(Path::new("")).to_path_buf();
            let Some(manifest) = ParsedManifest::parse(root, &rel_manifest, &crate_root) else {
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

                dependency_src_roots.insert(alias.clone(), target_manifest.src_root(&target_root));
                dependency_entry_files.insert(alias, target_manifest.crate_root_files.clone());
            }

            crates_by_root.insert(
                crate_root.clone(),
                RustWorkspaceCrate {
                    name,
                    src_root: manifest.src_root(crate_root),
                    crate_root_files: manifest.crate_root_files.clone(),
                    dependency_src_roots,
                    dependency_entry_files,
                },
            );
        }

        Self { crates_by_root }
    }
}

/// Resolves Rust `mod` and `use` items against the workspace cache.
pub(crate) struct RustResolver<'a> {
    root: &'a Path,
    workspace: &'a RustWorkspaceCache,
}

impl<'a> RustResolver<'a> {
    /// Create a new resolver for the given project root and workspace cache.
    pub(super) fn new(root: &'a Path, workspace: &'a RustWorkspaceCache) -> Self {
        Self { root, workspace }
    }

    /// Walk the source for `mod` and `use` items, resolve each against the
    /// crate context, and return the collected imports.
    fn resolve_source(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
        let crate_context = CrateContext::from_workspace(self.root, source_file, self.workspace);
        let mut imports = Vec::new();

        for item in collect_source_imports(source) {
            match item {
                SourceImport::Mod { name, line } => {
                    let resolved_path =
                        crate_context.resolve_mod_decl(self.root, source_file, &name);
                    let kind = if resolved_path.is_some() {
                        ImportKind::Relative
                    } else {
                        ImportKind::External
                    };

                    imports.push(ResolvedImport {
                        raw: format!("mod {name}"),
                        resolved_path,
                        kind,
                        names: ImportNames::new(vec![name]),
                        line,
                    });
                }
                SourceImport::Use { path, line } => {
                    let prefix = parse_use_prefix(&path);
                    let resolved_path = prefix
                        .as_deref()
                        .and_then(|value| crate_context.resolve_use(self.root, source_file, value));

                    // Multi-item brace group where the base alone didn't
                    // resolve: expand each item into its own ResolvedImport.
                    if resolved_path.is_none() {
                        if let Some(expanded) = expand_brace_imports(
                            &path,
                            line,
                            &crate_context,
                            self.root,
                            source_file,
                        ) {
                            imports.extend(expanded);
                            continue;
                        }
                    }

                    let kind = classify_use_kind(prefix.as_deref(), resolved_path.is_some());

                    let mut names = imported_names(&path);
                    if names.locals.is_empty() {
                        if let Some(ref resolved) = resolved_path {
                            if path.contains('*') {
                                names.locals =
                                    exported_symbol_names(self.root, resolved, CodeLanguage::Rust);
                            }
                        }
                    }

                    imports.push(ResolvedImport {
                        raw: path.clone(),
                        resolved_path,
                        kind,
                        names,
                        line,
                    });
                }
            }
        }

        imports
    }
}

impl LanguageResolver for RustResolver<'_> {
    /// Resolve all Rust imports in the given source file.
    fn resolve(&self, source_file: &Path, source: &str) -> Vec<ResolvedImport> {
        self.resolve_source(source_file, source)
    }
}

/// Contextual information about the crate containing a source file: its source
/// root, entry files, and dependency maps.
#[derive(Debug, Clone)]
struct CrateContext {
    src_root: PathBuf,
    crate_root_files: Vec<PathBuf>,
    dependency_src_roots: HashMap<String, PathBuf>,
    dependency_entry_files: HashMap<String, Vec<PathBuf>>,
}

impl CrateContext {
    /// Build crate context for `source_file` by finding its enclosing crate in
    /// the workspace cache. Falls back to sensible defaults when the file is
    /// outside any known crate.
    fn from_workspace(
        root: &Path,
        source_file: &Path,
        rust_workspace: &RustWorkspaceCache,
    ) -> Self {
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

        Self {
            src_root,
            crate_root_files,
            dependency_src_roots,
            dependency_entry_files,
        }
    }

    /// Resolve a `mod <name>;` declaration to a file path by looking for
    /// `<name>.rs` or `<name>/mod.rs` relative to the source file's module
    /// directory.
    fn resolve_mod_decl(&self, root: &Path, source_file: &Path, name: &str) -> Option<PathBuf> {
        let mut module_dir = source_file.parent().unwrap_or(Path::new("")).to_path_buf();
        if self
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

    /// Resolve a `use` path (after prefix extraction) to a file path, walking
    /// through `crate`, `self`, `super`, or external crate prefixes.
    fn resolve_use(&self, root: &Path, source_file: &Path, path: &str) -> Option<PathBuf> {
        let parts = path
            .split("::")
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }

        let mut cursor = 0usize;
        let (mut module, src_root, resolve_crate_entry, entry_files) =
            match parts.first().copied()? {
                "crate" => {
                    cursor += 1;
                    (Vec::new(), self.src_root.as_path(), false, Vec::new())
                }
                "self" => {
                    cursor += 1;
                    (
                        source_segments(source_file, &self.src_root)?,
                        self.src_root.as_path(),
                        false,
                        Vec::new(),
                    )
                }
                "super" => {
                    let mut module = source_segments(source_file, &self.src_root)?;
                    while cursor < parts.len() && parts[cursor] == "super" {
                        module.pop()?;
                        cursor += 1;
                    }
                    (module, self.src_root.as_path(), false, Vec::new())
                }
                crate_name => {
                    let dependency_root = self.dependency_src_roots.get(crate_name)?;
                    let entry_files = self
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
}

/// Walk the directory tree under `root` to find all `Cargo.toml` files.
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

/// Parse `[lib]` and `[[bin]]` sections to find crate root files.
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

/// Normalize and push a manifest entry path, deduplicating.
fn push_manifest_entry_path(files: &mut Vec<PathBuf>, crate_root: &Path, raw: &str) {
    let Some(path) = normalize_manifest_path(crate_root, raw) else {
        return;
    };
    if files.iter().all(|existing| existing != &path) {
        files.push(path);
    }
}

/// Return the default set of crate root file candidates.
fn default_crate_root_files(crate_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for candidate in ["src/lib.rs", "src/main.rs", "src/mod.rs"] {
        push_manifest_entry_path(&mut files, crate_root, candidate);
    }
    files
}

/// Join a raw path from the manifest with the crate root and normalize.
fn normalize_manifest_path(crate_root: &Path, raw: &str) -> Option<PathBuf> {
    let path = raw.trim();
    if path.is_empty() {
        return None;
    }
    super::normalize_rel_path(&crate_root.join(path))
}

/// Collect dependencies from `[dependencies]`, `[dev-dependencies]`, and
/// `[build-dependencies]` sections.
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

/// Parse a single dependency entry from a Cargo.toml table section.
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

/// Resolve a dependency to its crate root directory using its path, then
/// name lookup, then workspace fallback.
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

/// Look up a crate root by name, also trying the hyphenated variant.
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

/// Normalize a crate name into a valid Rust identifier (hyphens → underscores).
fn crate_import_name(raw: &str) -> Option<String> {
    normalize_identifier(&raw.replace('-', "_"))
}

/// Extract the use-path prefix before any brace group, comma, or `as` alias.
fn parse_use_prefix(path: &str) -> Option<String> {
    if let Some((prefix, body)) = split_brace_group(path.trim()) {
        let base = parse_use_head(prefix)?;
        if let Some(segment) = single_group_item_path(body) {
            return Some(format!("{base}::{segment}"));
        }
        return Some(base);
    }

    parse_use_head(path)
}

fn parse_use_head(path: &str) -> Option<String> {
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

fn single_group_item_path(body: &str) -> Option<String> {
    let mut items = body
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty() && *item != "*");

    let item = items.next()?;
    if items.next().is_some() || item == "self" {
        return None;
    }

    let item_path = item
        .split_once(" as ")
        .map(|(original, _)| original.trim())
        .unwrap_or(item)
        .trim();
    if item_path.is_empty() {
        return None;
    }

    Some(item_path.to_string())
}

/// Walk up from `source_file` to find the nearest directory containing a
/// `Cargo.toml`.
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

/// Classify a `use` path as relative, workspace, or external.
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

/// Resolve a module path to a `.rs` file or `mod.rs` under `src_root`.
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

/// Try to find a crate entry file (`lib.rs`, `main.rs`, `mod.rs`).
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

/// Generate the two candidate paths for a module: `<base>.rs` and `<base>/mod.rs`.
fn rust_file_candidates(src_root: &Path, module: &[String]) -> [PathBuf; 2] {
    let mut base = src_root.to_path_buf();
    for part in module {
        base.push(part);
    }
    [base.with_extension("rs"), base.join("mod.rs")]
}

/// Return `true` if the value looks like a lowercase module segment.
fn looks_like_module_segment(value: &str) -> bool {
    let candidate = value.strip_prefix("r#").unwrap_or(value);
    !candidate.is_empty()
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Derive the module path segments for a source file relative to its `src_root`.
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

/// Extract the imported names and alias map from a Rust `use` path, handling
/// brace groups, `as` aliases, and `self`/`*`.
fn imported_names(path: &str) -> ImportNames {
    let trimmed = path.trim();
    if let Some((prefix, body)) = split_brace_group(trimmed) {
        let mut result = ImportNames::default();
        for item in body.split(',') {
            let token = item.trim();
            if token.is_empty() || token == "*" {
                continue;
            }

            if token == "self" {
                if let Some(name) = last_segment(prefix) {
                    result.locals.push(name);
                }
                continue;
            }

            if let Some((original, alias)) = token.split_once(" as ") {
                if let Some(local_name) = last_segment(alias) {
                    if let Some(def_name) = last_segment(original) {
                        if local_name != def_name {
                            result.aliases.insert(local_name.clone(), def_name);
                        }
                    }
                    result.locals.push(local_name);
                }
            } else if let Some(name) = last_segment(token) {
                result.locals.push(name);
            }
        }
        result.locals = dedupe_names(result.locals);
        return result;
    }

    if let Some((original, alias)) = trimmed.split_once(" as ") {
        let local_name = last_segment(alias);
        let def_name = last_segment(original);
        let mut result = ImportNames::default();
        if let (Some(local), Some(def)) = (&local_name, &def_name) {
            if local != def {
                result.aliases.insert(local.clone(), def.clone());
            }
        }
        result.locals = local_name.into_iter().collect();
        return result;
    }

    ImportNames::new(
        last_segment(trimmed)
            .map(|name| vec![name])
            .unwrap_or_default(),
    )
}

/// Split a use path at the outermost `{...}` brace group.
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

/// Split the body of a brace group into individual items, respecting nested
/// braces so that `event::{self, Source}, Token` yields two items rather than
/// splitting inside the nested group.
fn split_brace_items(body: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;

    for (i, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let item = body[start..i].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = i + 1;
            }
            _ => {}
        }
    }

    let last = body[start..].trim();
    if !last.is_empty() {
        items.push(last);
    }

    items
}

/// Expand a multi-item brace group into per-item resolved imports. Returns
/// `None` when the path is not a multi-item brace group (fast-path exits).
fn expand_brace_imports(
    path: &str,
    line: usize,
    crate_context: &CrateContext,
    root: &Path,
    source_file: &Path,
) -> Option<Vec<ResolvedImport>> {
    let (prefix, body) = split_brace_group(path.trim())?;
    let base = parse_use_head(prefix)?;
    let items = split_brace_items(body);
    if items.len() < 2 {
        return None;
    }

    let mut result = Vec::new();
    for item in items {
        let full_path = format!("{base}::{item}");
        let item_prefix = parse_use_prefix(&full_path);
        let resolved_path = item_prefix
            .as_deref()
            .and_then(|value| crate_context.resolve_use(root, source_file, value));
        let kind = classify_use_kind(item_prefix.as_deref(), resolved_path.is_some());
        let names = imported_names(&full_path);

        result.push(ResolvedImport {
            raw: full_path,
            resolved_path,
            kind,
            names,
            line,
        });
    }

    Some(result)
}

/// Extract and normalize the last `::` segment from a use path.
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

/// Remove duplicate names while preserving order.
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
