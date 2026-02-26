# Spec: Multi-pass reference resolution

Detailed design for iss-0041.

## Current pipeline

`Indexer::build()` in `src/index/code.rs`:

```
load_code_files()           ← read source text
extract_symbols()           ← tree-sitter symbol extraction
build_symbol_lookup()       ← per-file: name → [SymbolKey]
build_reexport_lookup()     ← per-file: name → [ReexportTarget]
seed_definition_refs()      ← is_definition rows
link_usage_refs()           ← per-file: build ImportedBindings, scan, match
link_go_same_package_refs() ← Go-specific cross-file scan
normalize_symbol_refs()     ← sort + dedup
```

The problem is in `link_usage_refs()`: it builds `ImportedBindings` from
raw imports, scans for usages, and tries to match — all in one pass per
file. Re-export following happens reactively in `insert_usage_row()` when
a direct lookup fails. Qualified access is handled inside `UsageScanner`
but the binding name must already be in the import map.

When patterns compound (grouped import → module → re-export), the binding
name points to an intermediary file, the symbol isn't there, the re-export
lookup finds it, but the qualified access expansion hasn't run yet because
it's inside the scanner. There's no way to feed results back.

## New pipeline

```
load_code_files()           ← unchanged
extract_symbols()           ← unchanged
build_symbol_lookup()       ← unchanged
build_module_scopes()       ← NEW: per-file ModuleScope from imports
resolution_loop()           ← NEW: fixed-point enrichment of scopes
seed_definition_refs()      ← unchanged
link_usage_refs()           ← simplified: scopes already resolved
link_go_same_package_refs() ← unchanged
normalize_symbol_refs()     ← unchanged
```

### New data structures

#### `ModuleScope`

Replaces `ImportedBindings`. One per source file. Built from resolved
imports, then enriched during the resolution loop.

```rust
struct ModuleScope {
    /// Binding name → target files where the symbol might be defined.
    /// Populated initially from ResolvedImport.names.locals.
    bindings: HashMap<String, BTreeSet<PathBuf>>,

    /// Local alias → definition name (e.g. "B" → "Bar" from `import { Bar as B }`).
    aliases: HashMap<String, String>,

    /// Files from namespace/dot imports (Go dot imports) whose symbols
    /// are directly in scope.
    namespace_targets: Vec<PathBuf>,

    /// Files this module glob-imports from (Rust `use foo::*`, Python
    /// `from foo import *`). Used by propagate_glob_imports().
    glob_sources: Vec<GlobSource>,
}

struct GlobSource {
    target_file: PathBuf,
    /// For Python: if __all__ is defined, only these names. None = all public.
    all_filter: Option<Vec<String>>,
}
```

`ModuleScope` is built from `Vec<ResolvedImport>` — same data as
`ImportedBindings::from_imports()` but also captures glob sources.

#### `ExportedNames`

Answers "what does this file export?" — the key query for re-export
following and glob propagation.

```rust
struct ExportedNames {
    /// Symbols defined in this file (from symbol_lookup).
    defined: HashSet<String>,
    /// Symbols re-exported through this file (from reexport_lookup).
    reexported: HashMap<String, ReexportTarget>,
}
```

Cached per file. Cheap to build from existing `symbol_lookup` +
`reexport_lookup`.

### `build_module_scopes()`

Initial construction — runs once before the loop.

```rust
fn build_module_scopes(&mut self) {
    for (source_file, imports) in &self.code_imports {
        let scope = ModuleScope::from_imports(imports);
        self.module_scopes.insert(source_file.clone(), scope);
    }
}
```

`ModuleScope::from_imports()` is essentially today's
`ImportedBindings::from_imports()` plus:
- Collecting `GlobSource` entries for wildcard imports
- NOT expanding namespace symbols yet (that moves into the loop)

### `resolution_loop()`

The core change. Runs after `build_module_scopes()`, before usage scanning.

```rust
fn resolution_loop(&mut self) {
    loop {
        let mut changed = false;
        changed |= self.resolve_reexports();
        changed |= self.expand_qualified_access();
        changed |= self.propagate_glob_imports();
        if !changed { break; }
    }
}
```

Each sub-step is described below.

#### `resolve_reexports() → bool`

For each binding in each module scope, check if the target file actually
defines the symbol. If not, check the reexport lookup and update the
binding to point to the real definition site.

```rust
fn resolve_reexports(&mut self) -> bool {
    let mut changed = false;

    for (_source_file, scope) in &mut self.module_scopes {
        // Snapshot current bindings to avoid borrow issues.
        let entries: Vec<(String, Vec<PathBuf>)> = scope.bindings
            .iter()
            .map(|(name, targets)| (name.clone(), targets.iter().cloned().collect()))
            .collect();

        for (name, targets) in entries {
            let lookup_name = scope.definition_name(&name);
            for target_file in &targets {
                // Already points to a real definition? Skip.
                if self.symbol_exists(target_file, lookup_name) {
                    continue;
                }
                // Try following the re-export chain.
                if let Some(followed) = self.follow_reexport_target(target_file, lookup_name) {
                    // Update binding to point to the real definition.
                    scope.bindings
                        .entry(name.clone())
                        .or_default()
                        .insert(followed.target_file);
                    changed = true;
                }
            }
        }
    }

    changed
}
```

This is essentially what `insert_usage_row()` does today (the fallback
to `follow_reexport_target`), but lifted out of usage scanning and into
a dedicated pass that runs before scanning.

#### `expand_qualified_access() → bool`

For bindings that resolve to a module file (the file exists in
`code_files` but the binding name matches a directory module, not a
specific symbol), scan the source for qualified access patterns and
create new bindings for the accessed symbols.

```rust
fn expand_qualified_access(&mut self) -> bool {
    let mut changed = false;

    // Collect (source_file, binding_name, target_file) triples where
    // the binding points to a module, not a symbol.
    let module_bindings: Vec<(PathBuf, String, PathBuf)> = self.module_scopes
        .iter()
        .flat_map(|(source_file, scope)| {
            scope.bindings.iter().flat_map(move |(name, targets)| {
                targets.iter()
                    .filter(|target| self.is_module_file(target))
                    .map(move |target| (source_file.clone(), name.clone(), target.clone()))
            })
        })
        .collect();

    for (source_file, binding_name, module_file) in module_bindings {
        let facts = &self.code_files[&source_file];

        // Scan for `binding_name::Symbol` (Rust) or `binding_name.symbol`
        // (TS/Python/Go) patterns in the source file.
        let accessed = scan_qualified_symbols(
            facts.language, &facts.source, &binding_name,
        );

        let scope = self.module_scopes.get_mut(&source_file).unwrap();
        for symbol_name in accessed {
            if scope.bindings.contains_key(&symbol_name) {
                continue; // already have a binding for this name
            }
            scope.bindings
                .entry(symbol_name)
                .or_default()
                .insert(module_file.clone());
            changed = true;
        }
    }

    changed
}
```

`scan_qualified_symbols()` is a focused tree-sitter scan — lighter than
the full `UsageScanner`. It only looks for qualified access patterns
where the qualifier matches the binding name, and returns the set of
accessed symbol names. This is a new function.

`is_module_file()` heuristic: the target file is a module entry point
(`mod.rs`, `__init__.py`, `index.ts`) OR the binding name doesn't appear
in the target file's symbol_lookup (the binding is a module name, not a
symbol name).

#### `propagate_glob_imports() → bool`

For each glob source in each module scope, look up what the target file
exports and add those names to the importing module's scope.

```rust
fn propagate_glob_imports(&mut self) -> bool {
    let mut changed = false;

    // Snapshot glob sources to avoid borrow conflicts.
    let glob_work: Vec<(PathBuf, Vec<GlobSource>)> = self.module_scopes
        .iter()
        .filter(|(_, scope)| !scope.glob_sources.is_empty())
        .map(|(file, scope)| (file.clone(), scope.glob_sources.clone()))
        .collect();

    for (source_file, glob_sources) in glob_work {
        for glob in &glob_sources {
            let exported = self.exported_names(&glob.target_file);
            let names: Vec<String> = match &glob.all_filter {
                Some(all) => all.iter()
                    .filter(|n| exported.has(n))
                    .cloned()
                    .collect(),
                None => exported.public_names(),
            };

            let scope = self.module_scopes.get_mut(&source_file).unwrap();
            for name in names {
                if scope.bindings.contains_key(&name) {
                    continue;
                }
                scope.bindings
                    .entry(name)
                    .or_default()
                    .insert(glob.target_file.clone());
                changed = true;
            }
        }
    }

    changed
}
```

For Python: `all_filter` is populated from `__all__` parsing (existing
`extract_python_all()`). When `None`, all public (non-`_` prefix) symbols
from the target file are imported.

For Rust: `all_filter` is always `None` — `pub use foo::*` imports all
`pub` items. The `exported_names()` query already knows which symbols
are `pub` from symbol extraction.

#### `exported_names()`

The "what does this file export?" query. Built lazily, cached.

```rust
fn exported_names(&self, file: &Path) -> ExportedNames {
    let defined: HashSet<String> = self.symbol_lookup
        .get(file)
        .map(|by_name| by_name.keys().cloned().collect())
        .unwrap_or_default();

    let reexported: HashMap<String, ReexportTarget> = self.reexport_lookup
        .get(file)
        .map(|by_name| {
            by_name.iter()
                .flat_map(|(name, targets)| {
                    targets.first().map(|t| (name.clone(), t.clone()))
                })
                .collect()
        })
        .unwrap_or_default();

    ExportedNames { defined, reexported }
}
```

`public_names()` returns the union of `defined` keys and `reexported`
keys. For Rust, filter by `pub` visibility (available in `Symbol::vis`).
For Python, filter by no `_` prefix (unless `__all__` is present).

### Simplified `link_usage_refs()`

After the resolution loop, module scopes are fully resolved. Usage
scanning becomes simpler:

```rust
fn link_usage_refs(&mut self) -> Result<()> {
    for (source_file, scope) in &self.module_scopes {
        let facts = &self.code_files[source_file];
        if scope.is_empty() {
            continue;
        }

        let scanner = UsageScanner::new(facts.language, &facts.source, scope.names());
        let usages = scanner.collect()?;
        self.insert_usage_refs(source_file, facts, scope, usages);
    }
    Ok(())
}
```

No more inline `follow_reexport_target()` fallback in `insert_usage_row()`
— the bindings already point to the right files.

### `scan_qualified_symbols()` — new function

A focused tree-sitter scan that finds qualified access patterns for a
given binding name. Lighter than `UsageScanner` — doesn't need the full
imported_names set, just looks for patterns where the qualifier matches.

```rust
/// Scan source for `qualifier::Name` (Rust) or `qualifier.name`
/// (TS/Python/Go) and return the set of accessed symbol names.
fn scan_qualified_symbols(
    language: CodeLanguage,
    source: &str,
    qualifier: &str,
) -> HashSet<String> {
    // Parse tree, walk nodes, match qualified patterns where
    // the left side equals `qualifier`. Return the right-side names.
}
```

This is extracted from the existing `UsageScanner::qualified_usage_for_node()`
logic but decoupled from the full usage scan.

## Convergence

The fixed-point loop converges because:

1. Each iteration can only **add** bindings (never remove).
2. The set of possible bindings is finite (bounded by files × symbols).
3. If no bindings are added (`changed == false`), the loop terminates.

In practice, most codebases converge in 1-2 iterations. The pathological
case (deep re-export chains through modules) would take N iterations for
an N-hop chain, but real-world chains are ≤3 hops.

## What changes in `Indexer`

```rust
struct Indexer<'a> {
    root: &'a Path,
    code_imports: &'a BTreeMap<PathBuf, Vec<ResolvedImport>>,
    code_files: BTreeMap<PathBuf, CodeFileFacts>,
    code_symbols: BTreeMap<PathBuf, Vec<Symbol>>,
    symbol_lookup: HashMap<PathBuf, HashMap<String, Vec<SymbolKey>>>,
    reexport_lookup: HashMap<PathBuf, HashMap<String, Vec<ReexportTarget>>>,
    module_scopes: HashMap<PathBuf, ModuleScope>,    // NEW: replaces per-file ImportedBindings
    symbol_refs: HashMap<SymbolKey, Vec<SymbolRef>>,
}
```

```rust
fn build(mut self) -> Result<SymbolIndex> {
    self.load_code_files()?;
    self.extract_symbols()?;
    self.build_symbol_lookup();
    self.build_reexport_lookup();        // still needed — feeds resolution_loop
    self.build_module_scopes();          // NEW
    self.resolution_loop();              // NEW
    self.seed_definition_refs();
    self.link_usage_refs()?;             // simplified — uses resolved scopes
    self.link_go_same_package_refs()?;
    self.normalize_symbol_refs();
    Ok(SymbolIndex {
        symbols: self.code_symbols,
        refs: self.symbol_refs,
    })
}
```

## What gets deleted

- `ImportedBindings` struct — replaced by `ModuleScope`
- `ImportedBindings::from_imports()` — replaced by `ModuleScope::from_imports()`
- `ImportedBindings::expand_namespace_symbols()` — moved into `resolution_loop`
- Reactive `follow_reexport_target()` call in `insert_usage_row()` — no
  longer needed, bindings are pre-resolved
- `ReexportTarget` and `FollowedReexport` structs stay — still used by
  `reexport_lookup` and `resolve_reexports()`

## What stays unchanged

- `UsageScanner` — same logic, just gets better bindings as input
- `seed_definition_refs()` — unchanged
- `link_go_same_package_refs()` — orthogonal, unchanged
- `normalize_symbol_refs()` — unchanged
- All per-language resolvers in `src/resolve/*.rs` — phase 1 is untouched
- `extract_reexport_bindings()` and per-language `collect_reexports()` —
  still used to build `reexport_lookup`

## Testing

1. All 38 eval tests must pass.
2. Real-world validation:

| Project | Lang | Symbol | Before | After |
|---|---|---|---|---|
| mio | Rust | `Source` | 2 | 12+ |
| tokio | Rust | `Handle` | 14 | 20+ |
| n8n | TS | `Server` | 1 | 2+ |
| poetry | Python | `Locker` | 60 | 60+ |

3. No performance regression on cold builds (the loop adds minimal work
   since most bindings resolve in the first iteration).

## Implementation order

1. Add `ModuleScope` + `GlobSource` + `ExportedNames` structs
2. Implement `build_module_scopes()` from existing `ImportedBindings` logic
3. Implement `resolution_loop()` with just `resolve_reexports()` first
4. Verify mio `Source` improves
5. Add `expand_qualified_access()` + `scan_qualified_symbols()`
6. Verify mio fully resolves (grouped import → module → re-export)
7. Add `propagate_glob_imports()`
8. Simplify `link_usage_refs()` — remove inline re-export fallback
9. Delete `ImportedBindings`
10. Run full eval suite + real-world validation
