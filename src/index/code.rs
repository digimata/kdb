use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::lang::CodeLanguage;
use crate::resolve::{ReexportBinding, ResolvedImport, extract_reexport_bindings};
use crate::symbols::{Symbol, extract_symbols};

use super::scanner::{UsageScanner, scan_qualified_symbols};
use super::scope::{ExportedNames, FollowedReexport, GlobSource, ModuleScope, ReexportTarget};
use super::{SymbolKey, SymbolRef};

// ----------------------------------------
// src/index/code.rs
//
// pub struct SymbolIndex               L53
//   pub(super) fn build()              L62
// struct CodeFileFacts                 L71
//   fn new()                           L78
//   fn snippet_for_line()              L87
//   fn column_from_byte()              L94
// struct Indexer                      L107
//   fn new()                          L119
//   fn build()                        L132
//   fn load_code_files()              L153
//   fn extract_symbols()              L179
//   fn build_symbol_lookup()          L193
//   fn build_reexport_lookup()        L206
//   fn resolve_reexport_target()      L251
//   fn build_module_scopes()          L290
//   fn resolution_loop()              L314
//   fn resolve_reexports()            L328
//   fn expand_qualified_access()      L368
//   fn propagate_glob_imports()       L410
//   fn is_module_file()               L445
//   fn exported_names()               L457
//   fn follow_reexport_target()       L488
//   fn symbol_exists()                L533
//   fn symbol_keys()                  L540
//   fn seed_definition_refs()         L551
//   fn link_usage_refs()              L573
//   fn link_go_same_package_refs()    L597
//   fn insert_usage_refs()            L665
//   fn insert_usage_row()             L689
//   fn normalize_symbol_refs()        L714
// fn symbol_key()                     L721
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
        self.extract_symbols()?;
        self.build_symbol_lookup();
        self.build_reexport_lookup();
        self.build_module_scopes();
        self.resolution_loop();
        self.seed_definition_refs();
        self.link_usage_refs()?;
        self.link_go_same_package_refs()?;
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

            self.files
                .insert(rel_path.clone(), CodeFileFacts::new(language, source));
        }
        Ok(())
    }

    fn extract_symbols(&mut self) -> Result<()> {
        for (file, facts) in &self.files {
            let mut symbols = extract_symbols(facts.language, &facts.source)
                .with_context(|| format!("failed to extract symbols for {}", file.display()))?;
            symbols.sort_by(|left, right| {
                left.line
                    .cmp(&right.line)
                    .then_with(|| left.name.cmp(&right.name))
            });
            self.symbols.insert(file.clone(), symbols);
        }
        Ok(())
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
        let mut by_file = HashMap::new();

        for (file, facts) in &self.files {
            let Some(imports) = self.imports.get(file) else {
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

    fn link_usage_refs(&mut self) -> Result<()> {
        let scope_entries: Vec<(PathBuf, ModuleScope)> = self
            .module_scopes
            .iter()
            .map(|(file, scope)| (file.clone(), scope.clone()))
            .collect();

        for (source_file, scope) in scope_entries {
            let Some(facts) = self.files.get(&source_file).cloned() else {
                continue;
            };
            if scope.is_empty() {
                continue;
            }

            let scanner = UsageScanner::new(facts.language, &facts.source, scope.names());
            let usages = scanner.collect()?;
            self.insert_usage_refs(&source_file, &facts, &scope, usages);
        }
        Ok(())
    }

    /// Scan Go files for same-package references — symbols used across files
    /// in the same directory without an import statement.
    fn link_go_same_package_refs(&mut self) -> Result<()> {
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

        for (_dir, files) in &packages {
            if files.len() < 2 {
                continue;
            }

            for source_file in files {
                let Some(facts) = self.files.get(source_file).cloned() else {
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

                let scope = ModuleScope {
                    bindings: by_name
                        .into_iter()
                        .map(|(name, targets)| (name, targets.into_iter().collect()))
                        .collect(),
                    aliases: HashMap::new(),
                    namespace_targets: Vec::new(),
                    glob_sources: Vec::new(),
                };

                let scanner = UsageScanner::new(facts.language, &facts.source, scope.names());
                let usages = scanner.collect()?;
                self.insert_usage_refs(source_file, &facts, &scope, usages);
            }
        }

        Ok(())
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
