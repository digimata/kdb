use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::lang::CodeLanguage;
use crate::resolve::{ReexportBinding, ResolvedImport, extract_reexport_bindings};
use crate::symbols::{Symbol, extract_symbols_from_tree, parse_tree};

use super::scanner::{UsageScanner, scan_qualified_symbols};
use super::scope::{ExportedNames, FollowedReexport, GlobSource, ModuleScope, ReexportTarget};
use super::{SymbolKey, SymbolRef};

// ----------------------------------------
// src/index/code.rs
//
// pub struct SymbolIndex               L54
//   pub(super) fn build()              L63
// struct CodeFileFacts                 L72
//   fn new()                           L81
//   fn snippet_for_line()              L92
//   fn column_from_byte()              L99
// struct Indexer                      L112
//   fn new()                          L124
//   fn build()                        L137
//   fn load_code_files()              L158
//   fn extract_symbols()              L185
//   fn build_symbol_lookup()          L206
//   fn build_reexport_lookup()        L219
//   fn resolve_reexport_target()      L269
//   fn build_module_scopes()          L308
//   fn resolution_loop()              L332
//   fn resolve_reexports()            L346
//   fn expand_qualified_access()      L386
//   fn propagate_glob_imports()       L428
//   fn is_module_file()               L463
//   fn exported_names()               L475
//   fn follow_reexport_target()       L506
//   fn symbol_exists()                L551
//   fn symbol_keys()                  L558
//   fn seed_definition_refs()         L569
//   fn link_usage_refs()              L591
//   fn link_go_same_package_refs()    L618
//   fn insert_usage_refs()            L696
//   fn insert_usage_row()             L720
//   fn normalize_symbol_refs()        L745
// fn symbol_key()                     L752
// ----------------------------------------

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
    tree: tree_sitter::Tree,
}

impl CodeFileFacts {
    /// Parse source into a tree-sitter tree and pre-split lines for snippets.
    fn new(language: CodeLanguage, source: String) -> Result<Self> {
        let tree = parse_tree(language, &source)?;
        let lines = source.lines().map(ToString::to_string).collect();
        Ok(Self {
            language,
            source,
            lines,
            tree,
        })
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

#[derive(Debug)]
struct Indexer<'a> {
    root: &'a Path,
    imports: &'a BTreeMap<PathBuf, Vec<ResolvedImport>>,
    files: BTreeMap<PathBuf, CodeFileFacts>,
    symbols: BTreeMap<PathBuf, Vec<Symbol>>,
    symbol_lookup: HashMap<PathBuf, HashMap<String, Vec<SymbolKey>>>,
    reexport_lookup: HashMap<PathBuf, HashMap<String, Vec<ReexportTarget>>>,
    module_scopes: HashMap<PathBuf, ModuleScope>,
    symbol_refs: HashMap<SymbolKey, Vec<SymbolRef>>,
}

impl<'a> Indexer<'a> {
    fn new(root: &'a Path, code_imports: &'a BTreeMap<PathBuf, Vec<ResolvedImport>>) -> Self {
        Self {
            root,
            imports: code_imports,
            files: BTreeMap::new(),
            symbols: BTreeMap::new(),
            symbol_lookup: HashMap::new(),
            reexport_lookup: HashMap::new(),
            module_scopes: HashMap::new(),
            symbol_refs: HashMap::new(),
        }
    }

    fn build(mut self) -> Result<SymbolIndex> {
        self.load_code_files()?;
        self.extract_symbols();
        self.build_symbol_lookup();
        self.build_reexport_lookup();
        self.build_module_scopes();
        self.resolution_loop();
        self.seed_definition_refs();
        self.link_usage_refs();
        self.link_go_same_package_refs();
        self.normalize_symbol_refs();
        Ok(SymbolIndex {
            symbols: self.symbols,
            refs: self.symbol_refs,
        })
    }

    // -----------------------------------------------------------------------
    // Phase 1: Load and extract
    // -----------------------------------------------------------------------

    fn load_code_files(&mut self) -> Result<()> {
        for rel_path in self.imports.keys() {
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

            let facts = CodeFileFacts::new(language, source)
                .with_context(|| format!("failed to parse {}", rel_path.display()))?;
            self.files.insert(rel_path.clone(), facts);
        }
        Ok(())
    }

    fn extract_symbols(&mut self) {
        let results: Vec<_> = self
            .files
            .par_iter()
            .map(|(file, facts)| {
                let mut symbols =
                    extract_symbols_from_tree(facts.language, &facts.source, &facts.tree);
                symbols.sort_by(|left, right| {
                    left.line
                        .cmp(&right.line)
                        .then_with(|| left.name.cmp(&right.name))
                });
                (file.clone(), symbols)
            })
            .collect();

        for (file, symbols) in results {
            self.symbols.insert(file, symbols);
        }
    }

    fn build_symbol_lookup(&mut self) {
        for (file, symbols) in &self.symbols {
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
        let imports = &self.imports;
        let results: Vec<_> = self
            .files
            .par_iter()
            .filter_map(|(file, facts)| {
                let file_imports = imports.get(file)?;

                let bindings =
                    extract_reexport_bindings(file, &facts.source, facts.language, &facts.tree);
                if bindings.is_empty() {
                    return None;
                }

                let mut by_name: HashMap<String, BTreeSet<ReexportTarget>> = HashMap::new();
                for binding in bindings {
                    let Some(target_file) = Self::resolve_reexport_target(file_imports, &binding)
                    else {
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
                    return None;
                }

                let by_name = by_name
                    .into_iter()
                    .map(|(name, targets)| (name, targets.into_iter().collect()))
                    .collect();
                Some((file.clone(), by_name))
            })
            .collect();

        for (file, by_name) in results {
            self.reexport_lookup.insert(file, by_name);
        }
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

    // -----------------------------------------------------------------------
    // Phase 2: Module scopes and resolution loop
    // -----------------------------------------------------------------------

    /// Build per-file module scopes from resolved imports and expand namespace
    /// symbols (Go dot imports).
    fn build_module_scopes(&mut self) {
        for (source_file, imports) in self.imports {
            let mut scope = ModuleScope::from_imports(imports);

            // Expand namespace imports (Go dot imports) by adding all symbols
            // from target files into bindings.
            for target in &scope.namespace_targets.clone() {
                let Some(symbols_by_name) = self.symbol_lookup.get(target) else {
                    continue;
                };
                for name in symbols_by_name.keys() {
                    scope
                        .bindings
                        .entry(name.clone())
                        .or_default()
                        .insert(target.clone());
                }
            }

            self.module_scopes.insert(source_file.clone(), scope);
        }
    }

    /// Fixed-point resolution loop — enriches module scopes until stable.
    fn resolution_loop(&mut self) {
        loop {
            let mut changed = false;
            changed |= self.resolve_reexports();
            changed |= self.expand_qualified_access();
            changed |= self.propagate_glob_imports();
            if !changed {
                break;
            }
        }
    }

    /// For each binding in each scope, if the target file doesn't define the
    /// symbol, try following the re-export chain and update the binding.
    fn resolve_reexports(&mut self) -> bool {
        let mut changed = false;

        let scope_files: Vec<PathBuf> = self.module_scopes.keys().cloned().collect();
        for source_file in scope_files {
            let scope = &self.module_scopes[&source_file];
            let entries: Vec<(String, Vec<PathBuf>)> = scope
                .bindings
                .iter()
                .map(|(name, targets)| (name.clone(), targets.iter().cloned().collect()))
                .collect();

            for (name, targets) in entries {
                let lookup_name = self.module_scopes[&source_file]
                    .definition_name(&name)
                    .unwrap_or(&name)
                    .to_string();

                for target_file in &targets {
                    if self.symbol_exists(target_file, &lookup_name) {
                        continue;
                    }
                    if let Some(followed) = self.follow_reexport_target(target_file, &lookup_name) {
                        let scope = self.module_scopes.get_mut(&source_file).unwrap();
                        let inserted = scope
                            .bindings
                            .entry(name.clone())
                            .or_default()
                            .insert(followed.target_file);
                        changed |= inserted;
                    }
                }
            }
        }

        changed
    }

    /// For bindings pointing to module files, scan the source for qualified
    /// access patterns and create new bindings for accessed symbols.
    fn expand_qualified_access(&mut self) -> bool {
        let mut changed = false;

        // Collect (source_file, binding_name, target_file) triples where the
        // target looks like a module file. Two-phase to avoid borrow conflicts.
        let mut module_bindings = Vec::new();
        for (source_file, scope) in &self.module_scopes {
            for (name, targets) in &scope.bindings {
                for target in targets {
                    if self.is_module_file(target, name) {
                        module_bindings.push((source_file.clone(), name.clone(), target.clone()));
                    }
                }
            }
        }

        for (source_file, binding_name, module_file) in module_bindings {
            let Some(facts) = self.files.get(&source_file) else {
                continue;
            };

            let accessed = scan_qualified_symbols(facts.language, &facts.source, &binding_name);

            let scope = self.module_scopes.get_mut(&source_file).unwrap();
            for symbol_name in accessed {
                if scope.bindings.contains_key(&symbol_name) {
                    continue;
                }
                let inserted = scope
                    .bindings
                    .entry(symbol_name)
                    .or_default()
                    .insert(module_file.clone());
                changed |= inserted;
            }
        }

        changed
    }

    /// For each glob source in each scope, look up exported names from the
    /// target file and add them to the importing scope.
    fn propagate_glob_imports(&mut self) -> bool {
        let mut changed = false;

        let glob_work: Vec<(PathBuf, Vec<GlobSource>)> = self
            .module_scopes
            .iter()
            .filter(|(_, scope)| !scope.glob_sources.is_empty())
            .map(|(file, scope)| (file.clone(), scope.glob_sources.clone()))
            .collect();

        for (source_file, glob_sources) in glob_work {
            for glob in &glob_sources {
                let exported = self.exported_names(&glob.target_file);
                let names: Vec<String> = match &glob.all_filter {
                    Some(all) => all.iter().filter(|n| exported.has(n)).cloned().collect(),
                    None => exported.public_names(),
                };

                let scope = self.module_scopes.get_mut(&source_file).unwrap();
                for name in names {
                    let inserted = scope
                        .bindings
                        .entry(name)
                        .or_default()
                        .insert(glob.target_file.clone());
                    changed |= inserted;
                }
            }
        }

        changed
    }

    /// Heuristic: target is a module entry point or the binding name doesn't
    /// appear as a symbol in the target file.
    fn is_module_file(&self, target: &Path, binding_name: &str) -> bool {
        let file_name = target.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(
            file_name,
            "mod.rs" | "__init__.py" | "index.ts" | "index.js" | "index.tsx"
        ) {
            return true;
        }
        !self.symbol_exists(target, binding_name)
    }

    /// Build the exported names for a file from symbol_lookup + reexport_lookup.
    fn exported_names(&self, file: &Path) -> ExportedNames {
        let defined: HashSet<String> = self
            .symbol_lookup
            .get(file)
            .map(|by_name| by_name.keys().cloned().collect())
            .unwrap_or_default();

        let reexported: HashMap<String, ReexportTarget> = self
            .reexport_lookup
            .get(file)
            .map(|by_name| {
                by_name
                    .iter()
                    .flat_map(|(name, targets)| targets.first().map(|t| (name.clone(), t.clone())))
                    .collect()
            })
            .unwrap_or_default();

        ExportedNames {
            defined,
            reexported,
        }
    }

    // -----------------------------------------------------------------------
    // Re-export chain following
    // -----------------------------------------------------------------------

    /// Follow re-export chains across files until the actual symbol definition
    /// is found. Uses a visited set for cycle detection instead of an arbitrary
    /// depth limit.
    fn follow_reexport_target(
        &self,
        intermediary_file: &Path,
        lookup_name: &str,
    ) -> Option<FollowedReexport> {
        let mut current_file = intermediary_file.to_path_buf();
        let mut current_name = lookup_name.to_string();
        let mut visited = HashSet::new();

        loop {
            if !visited.insert((current_file.clone(), current_name.clone())) {
                return None; // cycle detected
            }

            let candidates = self
                .reexport_lookup
                .get(&current_file)
                .and_then(|by_name| by_name.get(&current_name))?;

            for candidate in candidates {
                if self.symbol_exists(&candidate.target_file, &candidate.definition_name) {
                    return Some(FollowedReexport {
                        target_file: candidate.target_file.clone(),
                        lookup_name: candidate.definition_name.clone(),
                    });
                }

                if candidate.definition_name != current_name
                    && self.symbol_exists(&candidate.target_file, &current_name)
                {
                    return Some(FollowedReexport {
                        target_file: candidate.target_file.clone(),
                        lookup_name: current_name.clone(),
                    });
                }
            }

            // Symbol not found at this level — follow the first candidate
            // deeper into the re-export chain.
            let first = &candidates[0];
            current_file = first.target_file.clone();
            current_name = first.definition_name.clone();
        }
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

    // -----------------------------------------------------------------------
    // Phase 3: Seed definitions and link usages
    // -----------------------------------------------------------------------

    fn seed_definition_refs(&mut self) {
        for (file, symbols) in &self.symbols {
            let Some(facts) = self.files.get(file) else {
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

    fn link_usage_refs(&mut self) {
        let scan_inputs: Vec<_> = self
            .module_scopes
            .iter()
            .filter(|(_, scope)| !scope.is_empty())
            .filter_map(|(file, scope)| {
                let facts = self.files.get(file)?;
                Some((file.clone(), facts.clone(), scope.clone()))
            })
            .collect();

        let scan_results: Vec<_> = scan_inputs
            .par_iter()
            .map(|(file, facts, scope)| {
                let scanner = UsageScanner::new(facts.language, &facts.source, scope.names());
                let usages = scanner.collect(&facts.tree);
                (file, facts, scope, usages)
            })
            .collect();

        for (source_file, facts, scope, usages) in scan_results {
            self.insert_usage_refs(source_file, facts, scope, usages);
        }
    }

    /// Scan Go files for same-package references — symbols used across files
    /// in the same directory without an import statement.
    fn link_go_same_package_refs(&mut self) {
        // Group Go files by parent directory (= Go package boundary).
        let mut packages: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for (file, facts) in &self.files {
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

        // Build (file, facts, scope) triples for all Go files needing scans.
        let mut scan_inputs = Vec::new();
        for (_dir, files) in &packages {
            if files.len() < 2 {
                continue;
            }

            for source_file in files {
                let Some(facts) = self.files.get(source_file) else {
                    continue;
                };

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

                let scope = ModuleScope {
                    bindings: by_name
                        .into_iter()
                        .map(|(name, targets)| (name, targets.into_iter().collect()))
                        .collect(),
                    aliases: HashMap::new(),
                    namespace_targets: Vec::new(),
                    glob_sources: Vec::new(),
                };

                scan_inputs.push((source_file.clone(), facts.clone(), scope));
            }
        }

        let scan_results: Vec<_> = scan_inputs
            .par_iter()
            .map(|(file, facts, scope)| {
                let scanner = UsageScanner::new(facts.language, &facts.source, scope.names());
                let usages = scanner.collect(&facts.tree);
                (file, facts, scope, usages)
            })
            .collect();

        for (source_file, facts, scope, usages) in scan_results {
            self.insert_usage_refs(source_file, facts, scope, usages);
        }
    }

    fn insert_usage_refs(
        &mut self,
        source_file: &Path,
        facts: &CodeFileFacts,
        scope: &ModuleScope,
        usages: Vec<super::scanner::IdentifierUsage>,
    ) {
        for usage in usages {
            let binding_key = usage.binding_name.as_deref().unwrap_or(&usage.name);
            let Some(target_files) = scope.targets(binding_key) else {
                continue;
            };

            let lookup_name = if usage.binding_name.is_some() {
                usage.name.as_str()
            } else {
                scope.definition_name(binding_key).unwrap_or(&usage.name)
            };
            for target_file in &target_files {
                self.insert_usage_row(source_file, facts, target_file, &usage, lookup_name);
            }
        }
    }

    fn insert_usage_row(
        &mut self,
        source_file: &Path,
        facts: &CodeFileFacts,
        target_file: &Path,
        usage: &super::scanner::IdentifierUsage,
        lookup_name: &str,
    ) {
        let Some(keys) = self.symbol_keys(target_file, lookup_name) else {
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

fn symbol_key(file: &Path, symbol: &Symbol) -> SymbolKey {
    SymbolKey {
        file: file.to_path_buf(),
        name: symbol.name.clone(),
        parent: symbol.parent.clone(),
        kind: symbol.kind,
        line: symbol.line,
    }
}
