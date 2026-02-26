use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

use crate::lang::CodeLanguage;
use crate::resolve::ResolvedImport;
use crate::symbols::{Symbol, extract_symbols, parse_tree, raw_node_text, walk_depth_first};

use super::{SymbolKey, SymbolRef};

// --------------------------------------
// src/index/code_refs.rs
//
// pub struct SymbolIndex             L54
//   pub(super) fn build()            L63
// struct CodeFileFacts               L72
//   fn new()                         L79
//   fn snippet_for_line()            L88
//   fn column_from_byte()            L95
// struct IdentifierUsage            L108
// struct ImportedBindings           L115
//   fn from_imports()               L120
//   fn is_empty()                   L146
//   fn names()                      L150
//   fn targets()                    L154
// struct Indexer                    L160
//   fn new()                        L170
//   fn build()                      L181
//   fn load_code_files()            L194
//   fn extract_symbols()            L220
//   fn build_symbol_lookup()        L234
//   fn seed_definition_refs()       L247
//   fn link_usage_refs()            L269
//   fn insert_usage_refs()          L292
//   fn insert_usage_row()           L309
//   fn normalize_symbol_refs()      L337
// struct UsageScanner               L345
//   fn new()                        L352
//   fn collect()                    L360
//   fn is_usage_identifier()        L395
// fn is_import_identifier()         L407
// fn is_import_node()               L418
// fn is_declaration_identifier()    L436
// fn node_is_field()                L484
// fn same_node()                    L490
// fn symbol_key()                   L496
// --------------------------------------

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
}

impl ImportedBindings {
    fn from_imports(imports: &[ResolvedImport]) -> Self {
        let mut by_name: HashMap<String, BTreeSet<PathBuf>> = HashMap::new();
        let mut aliases = HashMap::new();

        for import in imports {
            let Some(target_file) = import.resolved_path.as_ref() else {
                continue;
            };
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

        Self { by_name, aliases }
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

#[derive(Debug)]
struct Indexer<'a> {
    root: &'a Path,
    code_imports: &'a BTreeMap<PathBuf, Vec<ResolvedImport>>,
    code_files: BTreeMap<PathBuf, CodeFileFacts>,
    code_symbols: BTreeMap<PathBuf, Vec<Symbol>>,
    symbol_lookup: HashMap<PathBuf, HashMap<String, Vec<SymbolKey>>>,
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
            symbol_refs: HashMap::new(),
        }
    }

    fn build(mut self) -> Result<SymbolIndex> {
        self.load_code_files()?;
        self.extract_symbols()?;
        self.build_symbol_lookup();
        self.seed_definition_refs();
        self.link_usage_refs()?;
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
            let bindings = ImportedBindings::from_imports(&imports);
            if bindings.is_empty() {
                continue;
            }

            let scanner = UsageScanner::new(facts.language, &facts.source, bindings.names());
            let usages = scanner.collect()?;
            self.insert_usage_refs(&source_file, &facts, &bindings, usages);
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
        let Some(keys) = self
            .symbol_lookup
            .get(target_file)
            .and_then(|symbols_by_name| symbols_by_name.get(lookup_name))
        else {
            return;
        };

        let snippet = facts.snippet_for_line(usage.line);
        for key in keys {
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
            _ => None,
        }
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
            _ => false,
        }
    }

    fn is_usage_identifier(&self, kind: &str) -> bool {
        match self.language {
            CodeLanguage::Rust => matches!(kind, "identifier" | "type_identifier"),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
                matches!(kind, "identifier" | "type_identifier")
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
            | "parameter"
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
