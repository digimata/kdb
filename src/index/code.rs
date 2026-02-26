use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

use crate::lang::CodeLanguage;
use crate::resolve::{ReexportBinding, ResolvedImport, extract_reexport_bindings};
use crate::symbols::{Symbol, extract_symbols, parse_tree, raw_node_text, walk_depth_first};

use super::{SymbolKey, SymbolRef};

// -----------------------------------------
// src/index/code.rs
//
// pub struct SymbolIndex                L70
//   pub(super) fn build()               L79
// struct CodeFileFacts                  L88
//   fn new()                            L95
//   fn snippet_for_line()              L104
//   fn column_from_byte()              L111
// struct IdentifierUsage               L124
// struct ImportedBindings              L132
//   fn from_imports()                  L142
//   fn expand_namespace_symbols()      L181
//   fn is_empty()                      L198
//   fn names()                         L202
//   fn targets()                       L206
//   fn definition_name()               L211
// struct ReexportTarget                L217
// struct FollowedReexport              L223
// struct Indexer                       L229
//   fn new()                           L240
//   fn build()                         L252
//   fn load_code_files()               L266
//   fn extract_symbols()               L292
//   fn build_symbol_lookup()           L306
//   fn build_reexport_lookup()         L319
//   fn resolve_reexport_target()       L364
//   fn follow_reexport_target()        L397
//   fn symbol_exists()                 L428
//   fn symbol_keys()                   L435
//   fn seed_definition_refs()          L442
//   fn link_usage_refs()               L464
//   fn insert_usage_refs()             L488
//   fn insert_usage_row()              L514
//   fn normalize_symbol_refs()         L547
// struct UsageScanner                  L555
//   fn new()                           L562
//   fn collect()                       L570
//   fn qualified_usage_for_node()      L616
//   fn member_expression_usage()       L641
//   fn go_qualified_usage()            L672
//   fn is_part_of_qualified_usage()    L692
//   fn is_usage_identifier()           L703
// fn go_qualified_nodes()              L715
// fn is_go_qualified_binding_node()    L729
// fn is_member_object_node()           L747
// fn is_import_identifier()            L759
// fn is_import_node()                  L770
// fn is_declaration_identifier()       L788
// fn node_is_field()                   L847
// fn same_node()                       L853
// fn symbol_key()                      L859
// -----------------------------------------

/// Declaration symbols and inbound references built for `kdb refs -s`.
#[derive(Debug, Clone, Default)]
pub struct SymbolIndex {
    /// Per-code-file declaration symbols.
    pub symbols: BTreeMap<PathBuf, Vec<Symbol>>,
    /// Inbound code references grouped by definition symbol.
    pub refs: HashMap<SymbolKey, Vec<SymbolRef>>,
}

impl SymbolIndex {
    /// Build from a pre-computed code import map.
    pub(super) fn build(
        root: &Path,
        code_imports: &BTreeMap<PathBuf, Vec<ResolvedImport>>,
    ) -> Result<Self> {
        Indexer::new(root, code_imports).build()
    }
}

#[derive(Debug, Clone)]
struct CodeFileFacts {
    language: CodeLanguage,
    source: String,
    lines: Vec<String>,
}

impl CodeFileFacts {
    fn new(language: CodeLanguage, source: String) -> Self {
        let lines = source.lines().map(ToString::to_string).collect();
        Self {
            language,
            source,
            lines,
        }
    }

    fn snippet_for_line(&self, line: usize) -> String {
        self.lines
            .get(line.saturating_sub(1))
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn column_from_byte(&self, start_byte: usize) -> usize {
        let bytes = self.source.as_bytes();
        let safe_start = start_byte.min(bytes.len());
        let line_start = bytes[..safe_start]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        safe_start.saturating_sub(line_start) + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct IdentifierUsage {
    name: String,
    binding_name: Option<String>,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Default)]
struct ImportedBindings {
    by_name: HashMap<String, Vec<PathBuf>>,
    /// Maps local alias → definition name across all imports for this file.
    aliases: HashMap<String, String>,
    /// Target files from namespace/dot imports whose exported symbols are
    /// directly in scope (Go dot imports).
    namespace_targets: Vec<PathBuf>,
}

impl ImportedBindings {
    fn from_imports(imports: &[ResolvedImport]) -> Self {
        let mut by_name: HashMap<String, BTreeSet<PathBuf>> = HashMap::new();
        let mut aliases = HashMap::new();
        let mut namespace_targets = BTreeSet::new();

        for import in imports {
            let Some(target_file) = import.resolved_path.as_ref() else {
                continue;
            };
            if import.names.is_namespace {
                namespace_targets.insert(target_file.clone());
            }
            for name in &import.names.locals {
                if name.is_empty() {
                    continue;
                }
                by_name
                    .entry(name.clone())
                    .or_default()
                    .insert(target_file.clone());
            }
            aliases.extend(import.names.aliases.clone());
        }

        let by_name = by_name
            .into_iter()
            .map(|(name, targets)| (name, targets.into_iter().collect()))
            .collect();

        Self {
            by_name,
            aliases,
            namespace_targets: namespace_targets.into_iter().collect(),
        }
    }

    /// Expand namespace imports by adding all symbols from target files into
    /// `by_name`. Handles Go dot imports where unqualified identifiers
    /// reference the imported package's symbols.
    fn expand_namespace_symbols(
        &mut self,
        symbol_lookup: &HashMap<PathBuf, HashMap<String, Vec<SymbolKey>>>,
    ) {
        for target in &self.namespace_targets {
            let Some(symbols_by_name) = symbol_lookup.get(target) else {
                continue;
            };
            for name in symbols_by_name.keys() {
                self.by_name
                    .entry(name.clone())
                    .or_default()
                    .push(target.clone());
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    fn names(&self) -> HashSet<String> {
        self.by_name.keys().cloned().collect()
    }

    fn targets(&self, name: &str) -> Option<&[PathBuf]> {
        self.by_name.get(name).map(Vec::as_slice)
    }

    /// Return the definition name for a local alias, if one exists.
    fn definition_name(&self, local: &str) -> Option<&str> {
        self.aliases.get(local).map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ReexportTarget {
    target_file: PathBuf,
    definition_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FollowedReexport {
    target_file: PathBuf,
    lookup_name: String,
}

#[derive(Debug)]
struct Indexer<'a> {
    root: &'a Path,
    code_imports: &'a BTreeMap<PathBuf, Vec<ResolvedImport>>,
    code_files: BTreeMap<PathBuf, CodeFileFacts>,
    code_symbols: BTreeMap<PathBuf, Vec<Symbol>>,
    symbol_lookup: HashMap<PathBuf, HashMap<String, Vec<SymbolKey>>>,
    reexport_lookup: HashMap<PathBuf, HashMap<String, Vec<ReexportTarget>>>,
    symbol_refs: HashMap<SymbolKey, Vec<SymbolRef>>,
}

impl<'a> Indexer<'a> {
    fn new(root: &'a Path, code_imports: &'a BTreeMap<PathBuf, Vec<ResolvedImport>>) -> Self {
        Self {
            root,
            code_imports,
            code_files: BTreeMap::new(),
            code_symbols: BTreeMap::new(),
            symbol_lookup: HashMap::new(),
            reexport_lookup: HashMap::new(),
            symbol_refs: HashMap::new(),
        }
    }

    fn build(mut self) -> Result<SymbolIndex> {
        self.load_code_files()?;
        self.extract_symbols()?;
        self.build_symbol_lookup();
        self.build_reexport_lookup();
        self.seed_definition_refs();
        self.link_usage_refs()?;
        self.link_go_same_package_refs()?;
        self.normalize_symbol_refs();
        Ok(SymbolIndex {
            symbols: self.code_symbols,
            refs: self.symbol_refs,
        })
    }

    fn load_code_files(&mut self) -> Result<()> {
        for rel_path in self.code_imports.keys() {
            let Some(language) = CodeLanguage::from_path(rel_path) else {
                continue;
            };

            let abs_path = self.root.join(rel_path);
            let source = match fs::read_to_string(&abs_path) {
                Ok(source) => source,
                Err(error)
                    if matches!(error.kind(), ErrorKind::InvalidData | ErrorKind::NotFound) =>
                {
                    continue;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to read {}", rel_path.display()));
                }
            };

            self.code_files
                .insert(rel_path.clone(), CodeFileFacts::new(language, source));
        }
        Ok(())
    }

    fn extract_symbols(&mut self) -> Result<()> {
        for (file, facts) in &self.code_files {
            let mut symbols = extract_symbols(facts.language, &facts.source)
                .with_context(|| format!("failed to extract symbols for {}", file.display()))?;
            symbols.sort_by(|left, right| {
                left.line
                    .cmp(&right.line)
                    .then_with(|| left.name.cmp(&right.name))
            });
            self.code_symbols.insert(file.clone(), symbols);
        }
        Ok(())
    }

    fn build_symbol_lookup(&mut self) {
        for (file, symbols) in &self.code_symbols {
            let mut by_name: HashMap<String, Vec<SymbolKey>> = HashMap::new();
            for symbol in symbols {
                by_name
                    .entry(symbol.name.clone())
                    .or_default()
                    .push(symbol_key(file, symbol));
            }
            self.symbol_lookup.insert(file.clone(), by_name);
        }
    }

    fn build_reexport_lookup(&mut self) {
        let mut by_file = HashMap::new();

        for (file, facts) in &self.code_files {
            let Some(imports) = self.code_imports.get(file) else {
                continue;
            };

            let bindings = extract_reexport_bindings(file, &facts.source, facts.language);
            if bindings.is_empty() {
                continue;
            }

            let mut by_name: HashMap<String, BTreeSet<ReexportTarget>> = HashMap::new();
            for binding in bindings {
                let Some(target_file) = Self::resolve_reexport_target(imports, &binding) else {
                    continue;
                };
                if target_file == *file {
                    continue;
                }

                by_name
                    .entry(binding.exported_name)
                    .or_default()
                    .insert(ReexportTarget {
                        target_file,
                        definition_name: binding.definition_name,
                    });
            }

            if by_name.is_empty() {
                continue;
            }

            let by_name = by_name
                .into_iter()
                .map(|(name, targets)| (name, targets.into_iter().collect()))
                .collect();
            by_file.insert(file.clone(), by_name);
        }

        self.reexport_lookup = by_file;
    }

    fn resolve_reexport_target(
        imports: &[ResolvedImport],
        binding: &ReexportBinding,
    ) -> Option<PathBuf> {
        imports
            .iter()
            .find_map(|import| {
                if import.line == binding.line && import.raw == binding.raw_specifier {
                    import.resolved_path.clone()
                } else {
                    None
                }
            })
            .or_else(|| {
                imports.iter().find_map(|import| {
                    if import.raw == binding.raw_specifier {
                        import.resolved_path.clone()
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                imports.iter().find_map(|import| {
                    if import.line == binding.line {
                        import.resolved_path.clone()
                    } else {
                        None
                    }
                })
            })
    }

    fn follow_reexport_target(
        &self,
        intermediary_file: &Path,
        lookup_name: &str,
    ) -> Option<FollowedReexport> {
        let candidates = self
            .reexport_lookup
            .get(intermediary_file)
            .and_then(|by_name| by_name.get(lookup_name))?;

        for candidate in candidates {
            if self.symbol_exists(&candidate.target_file, &candidate.definition_name) {
                return Some(FollowedReexport {
                    target_file: candidate.target_file.clone(),
                    lookup_name: candidate.definition_name.clone(),
                });
            }

            if candidate.definition_name != lookup_name
                && self.symbol_exists(&candidate.target_file, lookup_name)
            {
                return Some(FollowedReexport {
                    target_file: candidate.target_file.clone(),
                    lookup_name: lookup_name.to_string(),
                });
            }
        }

        None
    }

    fn symbol_exists(&self, file: &Path, name: &str) -> bool {
        self.symbol_lookup
            .get(file)
            .and_then(|symbols_by_name| symbols_by_name.get(name))
            .is_some()
    }

    fn symbol_keys(&self, file: &Path, name: &str) -> Option<Vec<SymbolKey>> {
        self.symbol_lookup
            .get(file)
            .and_then(|symbols_by_name| symbols_by_name.get(name))
            .cloned()
    }

    fn seed_definition_refs(&mut self) {
        for (file, symbols) in &self.code_symbols {
            let Some(facts) = self.code_files.get(file) else {
                continue;
            };

            for symbol in symbols {
                let row = SymbolRef {
                    source_file: file.clone(),
                    line: symbol.line,
                    column: facts.column_from_byte(symbol.start_byte),
                    snippet: facts.snippet_for_line(symbol.line),
                    is_definition: true,
                };
                self.symbol_refs
                    .entry(symbol_key(file, symbol))
                    .or_default()
                    .push(row);
            }
        }
    }

    fn link_usage_refs(&mut self) -> Result<()> {
        let import_entries = self
            .code_imports
            .iter()
            .map(|(source_file, imports)| (source_file.clone(), imports.clone()))
            .collect::<Vec<_>>();

        for (source_file, imports) in import_entries {
            let Some(facts) = self.code_files.get(&source_file).cloned() else {
                continue;
            };
            let mut bindings = ImportedBindings::from_imports(&imports);
            bindings.expand_namespace_symbols(&self.symbol_lookup);
            if bindings.is_empty() {
                continue;
            }

            let scanner = UsageScanner::new(facts.language, &facts.source, bindings.names());
            let usages = scanner.collect()?;
            self.insert_usage_refs(&source_file, &facts, &bindings, usages);
        }
        Ok(())
    }

    /// Scan Go files for same-package references — symbols used across files
    /// in the same directory without an import statement.
    fn link_go_same_package_refs(&mut self) -> Result<()> {
        // Group Go files by parent directory (= Go package boundary).
        let mut packages: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for (file, facts) in &self.code_files {
            if facts.language != CodeLanguage::Go {
                continue;
            }
            let dir = file.parent().unwrap_or(Path::new("")).to_path_buf();
            packages.entry(dir).or_default().push(file.clone());
        }

        // Snapshot symbol names per file to avoid holding refs into self.
        let pkg_symbol_names: HashMap<PathBuf, Vec<String>> = self
            .symbol_lookup
            .iter()
            .map(|(file, by_name)| (file.clone(), by_name.keys().cloned().collect()))
            .collect();

        for (_dir, files) in &packages {
            if files.len() < 2 {
                continue;
            }

            for source_file in files {
                let Some(facts) = self.code_files.get(source_file).cloned() else {
                    continue;
                };

                // Build bindings from all *other* files in the same package.
                let mut by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
                for def_file in files {
                    if def_file == source_file {
                        continue;
                    }
                    let Some(names) = pkg_symbol_names.get(def_file) else {
                        continue;
                    };
                    for name in names {
                        by_name
                            .entry(name.clone())
                            .or_default()
                            .push(def_file.clone());
                    }
                }

                if by_name.is_empty() {
                    continue;
                }

                let imported_names: HashSet<String> = by_name.keys().cloned().collect();
                let bindings = ImportedBindings {
                    by_name,
                    aliases: HashMap::new(),
                    namespace_targets: Vec::new(),
                };

                let scanner =
                    UsageScanner::new(facts.language, &facts.source, imported_names);
                let usages = scanner.collect()?;
                self.insert_usage_refs(source_file, &facts, &bindings, usages);
            }
        }

        Ok(())
    }

    fn insert_usage_refs(
        &mut self,
        source_file: &Path,
        facts: &CodeFileFacts,
        bindings: &ImportedBindings,
        usages: Vec<IdentifierUsage>,
    ) {
        for usage in usages {
            let binding_key = usage.binding_name.as_deref().unwrap_or(&usage.name);
            let target_files = bindings.targets(binding_key);

            let Some(target_files) = target_files else {
                continue;
            };

            let lookup_name = if usage.binding_name.is_some() {
                usage.name.as_str()
            } else {
                bindings.definition_name(binding_key).unwrap_or(&usage.name)
            };
            for target_file in target_files {
                self.insert_usage_row(source_file, facts, target_file, &usage, lookup_name);
            }
        }
    }

    fn insert_usage_row(
        &mut self,
        source_file: &Path,
        facts: &CodeFileFacts,
        target_file: &Path,
        usage: &IdentifierUsage,
        lookup_name: &str,
    ) {
        let mut keys = self.symbol_keys(target_file, lookup_name);
        if keys.is_none() {
            keys = self
                .follow_reexport_target(target_file, lookup_name)
                .and_then(|followed| {
                    self.symbol_keys(&followed.target_file, &followed.lookup_name)
                });
        }
        let Some(keys) = keys else {
            return;
        };

        let snippet = facts.snippet_for_line(usage.line);
        for key in &keys {
            let row = SymbolRef {
                source_file: source_file.to_path_buf(),
                line: usage.line,
                column: usage.column,
                snippet: snippet.clone(),
                is_definition: false,
            };
            self.symbol_refs.entry(key.clone()).or_default().push(row);
        }
    }

    fn normalize_symbol_refs(&mut self) {
        for refs in self.symbol_refs.values_mut() {
            super::refs::normalize_symbol_refs(refs);
        }
    }
}

#[derive(Debug)]
struct UsageScanner {
    language: CodeLanguage,
    source: String,
    imported_names: HashSet<String>,
}

impl UsageScanner {
    fn new(language: CodeLanguage, source: &str, imported_names: HashSet<String>) -> Self {
        Self {
            language,
            source: source.to_string(),
            imported_names,
        }
    }

    fn collect(&self) -> Result<Vec<IdentifierUsage>> {
        if self.imported_names.is_empty() {
            return Ok(Vec::new());
        }

        let tree = parse_tree(self.language, &self.source)?;
        let source_bytes = self.source.as_bytes();
        let mut usages = Vec::new();

        walk_depth_first(tree.root_node(), |node| {
            if let Some(usage) = self.qualified_usage_for_node(node, source_bytes) {
                usages.push(usage);
                return;
            }

            if !self.is_usage_identifier(node.kind()) {
                return;
            }

            if self.is_part_of_qualified_usage(node) {
                return;
            }

            let Some(name) = raw_node_text(node, source_bytes) else {
                return;
            };
            if !self.imported_names.contains(name) {
                return;
            }
            if is_import_identifier(node) || is_declaration_identifier(node) {
                return;
            }

            usages.push(IdentifierUsage {
                name: name.to_string(),
                binding_name: None,
                line: node.start_position().row + 1,
                column: node.start_position().column + 1,
            });
        });

        usages.sort();
        usages.dedup();
        Ok(usages)
    }

    fn qualified_usage_for_node(
        &self,
        node: Node<'_>,
        source_bytes: &[u8],
    ) -> Option<IdentifierUsage> {
        match self.language {
            CodeLanguage::Go => self.go_qualified_usage(node, source_bytes),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => self
                .member_expression_usage(
                    node,
                    source_bytes,
                    "member_expression",
                    "object",
                    "property",
                ),
            CodeLanguage::Python => {
                self.member_expression_usage(node, source_bytes, "attribute", "object", "attribute")
            }
            _ => None,
        }
    }

    /// Handle qualified access via member expressions (TS/JS `obj.prop`,
    /// Python `obj.attr`). When the object matches a namespace import binding,
    /// the property/attribute is treated as a usage from the target module.
    fn member_expression_usage(
        &self,
        node: Node<'_>,
        source_bytes: &[u8],
        parent_kind: &str,
        object_field: &str,
        property_field: &str,
    ) -> Option<IdentifierUsage> {
        if node.kind() != parent_kind {
            return None;
        }
        let object_node = node.child_by_field_name(object_field)?;
        let property_node = node.child_by_field_name(property_field)?;

        let binding_name = raw_node_text(object_node, source_bytes)?;
        if !self.imported_names.contains(binding_name) {
            return None;
        }
        if is_import_identifier(object_node) {
            return None;
        }

        let name = raw_node_text(property_node, source_bytes)?;
        Some(IdentifierUsage {
            name: name.to_string(),
            binding_name: Some(binding_name.to_string()),
            line: property_node.start_position().row + 1,
            column: property_node.start_position().column + 1,
        })
    }

    fn go_qualified_usage(&self, node: Node<'_>, source_bytes: &[u8]) -> Option<IdentifierUsage> {
        let (binding_node, symbol_node) = go_qualified_nodes(node)?;
        let binding_name = raw_node_text(binding_node, source_bytes)?;
        if !self.imported_names.contains(binding_name) {
            return None;
        }

        if is_import_identifier(symbol_node) || is_declaration_identifier(symbol_node) {
            return None;
        }

        let name = raw_node_text(symbol_node, source_bytes)?;
        Some(IdentifierUsage {
            name: name.to_string(),
            binding_name: Some(binding_name.to_string()),
            line: symbol_node.start_position().row + 1,
            column: symbol_node.start_position().column + 1,
        })
    }

    fn is_part_of_qualified_usage(&self, node: Node<'_>) -> bool {
        match self.language {
            CodeLanguage::Go => is_go_qualified_binding_node(node),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
                is_member_object_node(node, "member_expression", "object")
            }
            CodeLanguage::Python => is_member_object_node(node, "attribute", "object"),
            _ => false,
        }
    }

    fn is_usage_identifier(&self, kind: &str) -> bool {
        match self.language {
            CodeLanguage::Rust => matches!(kind, "identifier" | "type_identifier"),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
                matches!(kind, "identifier" | "type_identifier" | "jsx_identifier")
            }
            CodeLanguage::Python => kind == "identifier",
            CodeLanguage::Go => matches!(kind, "identifier" | "type_identifier"),
        }
    }
}

fn go_qualified_nodes(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    match node.kind() {
        "selector_expression" => Some((
            node.child_by_field_name("operand")?,
            node.child_by_field_name("field")?,
        )),
        "qualified_type" => Some((
            node.child_by_field_name("package")?,
            node.child_by_field_name("name")?,
        )),
        _ => None,
    }
}

fn is_go_qualified_binding_node(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    match parent.kind() {
        "selector_expression" => parent
            .child_by_field_name("operand")
            .is_some_and(|operand| same_node(operand, node)),
        "qualified_type" => parent
            .child_by_field_name("package")
            .is_some_and(|package| same_node(package, node)),
        _ => false,
    }
}

/// Check if `node` is the object side of a member expression / attribute
/// access, which means it should be skipped as a bare usage.
fn is_member_object_node(node: Node<'_>, parent_kind: &str, object_field: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != parent_kind {
        return false;
    }
    parent
        .child_by_field_name(object_field)
        .is_some_and(|obj| same_node(obj, node))
}

fn is_import_identifier(node: Node<'_>) -> bool {
    let mut cursor = Some(node);
    while let Some(current) = cursor {
        if is_import_node(current.kind()) {
            return true;
        }
        cursor = current.parent();
    }
    false
}

fn is_import_node(kind: &str) -> bool {
    matches!(
        kind,
        "import_statement"
            | "import_clause"
            | "import_specifier"
            | "namespace_import"
            | "named_imports"
            | "import_declaration"
            | "import_spec"
            | "import_from_statement"
            | "aliased_import"
            | "use_declaration"
            | "extern_crate_declaration"
            | "mod_item"
    )
}

fn is_declaration_identifier(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if parent.kind() == "qualified_type" {
        return false;
    }

    // JSX element tags use the `name` field, but they're usages, not declarations.
    if matches!(
        parent.kind(),
        "jsx_opening_element" | "jsx_closing_element" | "jsx_self_closing_element"
    ) {
        return false;
    }

    if node_is_field(parent, "name", node)
        || node_is_field(parent, "alias", node)
        || node_is_field(parent, "parameter", node)
        || node_is_field(parent, "pattern", node)
    {
        return true;
    }

    matches!(
        parent.kind(),
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_definition"
            | "method_declaration"
            | "class_definition"
            | "class_declaration"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "type_spec"
            | "interface_declaration"
            | "type_alias_declaration"
            | "variable_declarator"
            | "lexical_declaration"
            | "variable_declaration"
            | "const_spec"
            | "var_spec"
            | "short_var_declaration"
            | "parameters"
            | "formal_parameters"
            | "parameter_list"
            | "required_parameter"
            | "optional_parameter"
            | "typed_parameter"
            | "receiver"
            | "pair_pattern"
            | "assignment_pattern"
    )
}

fn node_is_field(parent: Node<'_>, field: &str, node: Node<'_>) -> bool {
    parent
        .child_by_field_name(field)
        .is_some_and(|field_node| same_node(field_node, node))
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
        && left.kind() == right.kind()
}

fn symbol_key(file: &Path, symbol: &Symbol) -> SymbolKey {
    SymbolKey {
        file: file.to_path_buf(),
        name: symbol.name.clone(),
        parent: symbol.parent.clone(),
        kind: symbol.kind,
        line: symbol.line,
    }
}
