---
id: 28
title: Code symbol references (refs -s)
status: in_progress
priority: high
labels:
  - feat
  - refs
---

# ISS-0028 :: Code symbol references (`refs -s`)

## Intent

Find all references to a code symbol across the project using import-resolved precision.

## CLI

```
kdb refs <file>                       # markdown inbound refs (current)
kdb refs <file>#<heading>             # markdown heading refs (current)
kdb refs <file> -s <name>             # code symbol references (new)
kdb refs <file> -s <name> --count     # just the count
```

Example:

```
$ kdb refs src/project/root.rs -s find_root
src/project/root.rs:34:8            pub fn find_root(start: &Path) -> Result<PathBuf> {
src/cmd.rs:58:26                    ProjectContext::discover(&start)
src/lsp/backend.rs:42:18           root::find_root(&file_abs)
tests/root.rs:15:10                 find_root(&dir)
```

Definition site is included by default (like Zed's `include_declaration: true`).

## Design

### Why import-resolved?

Text search gives false positives — it can't distinguish identifiers from strings, comments, or identically-named symbols in unrelated scopes. Import resolution means: when file A imports `handle` from file B, and we see `handle(...)` in file A, we know it's a reference to B's `handle`, not some other `handle`.

### Approach

Build on the existing `CodeIndex` and symbol extraction to add identifier usage tracking and reference resolution at index time.

### What exists today

| Component | Location | What it provides |
|---|---|---|
| `CodeIndex` | `index/mod.rs` | Per-file `ResolvedImport` with resolved paths + imported names |
| `Symbol` extraction | `symbols/extract/` | Tree-sitter-based extraction of declarations (name, parent, kind, span, visibility) |
| `ResolvedImport` | `resolve/mod.rs` | `{ raw, resolved_path, kind, names, line }` — knows which names come from which files |
| Language resolvers | `resolve/{go,python,rust,tsjs}.rs` | Workspace-aware import → file path resolution |
| Markdown refs | `index/refs.rs` | `collect_inbound()` + `LinkRef` pattern — model for code refs |
| `ProjectIndex` | `index/mod.rs` | Combined `{ vault, code }` wrapper |

### What's new

**1. Identifier usage extraction** — for each code file, extract all identifier usages from the AST (not just declarations). A usage is: a name node in a call expression, type annotation, field access, etc. that isn't itself a declaration.

**2. Reference resolution** — match identifier usages against the file's import map (`ResolvedImport.names` → `ResolvedImport.resolved_path`). If an identifier name matches an imported name, and the target file contains a symbol with that name, it's a confirmed reference.

**3. `SymbolRef` storage** — new fields on `CodeIndex`:

```rust
/// Stored on CodeIndex
pub code_symbols: BTreeMap<PathBuf, Vec<Symbol>>,
pub symbol_refs: HashMap<SymbolKey, Vec<SymbolRef>>,
```

```rust
pub struct SymbolKey {
    pub file: PathBuf,           // definition file (rel path)
    pub name: String,            // symbol name
    pub parent: Option<String>,  // parent (struct/class for methods)
    pub kind: SymbolKind,        // function, struct, class, etc.
    pub line: usize,             // tie-breaker for overloaded names
}

pub struct SymbolRef {
    pub source_file: PathBuf,    // file containing the reference
    pub line: usize,             // 1-based line
    pub column: usize,           // 1-based column
    pub snippet: String,         // trimmed source line for display
    pub is_definition: bool,     // true for the declaration site itself
}
```

### Build pipeline

Current `CodeIndex::build()` does steps 1-2. Steps 3-5 are new.

1. Walk project, discover code files (existing)
2. Resolve imports per file → `code_imports` (existing)
3. **Extract symbols per file** → `code_symbols` (reuse `symbols::extract_symbols`)
4. **Extract identifier usages per file** (new tree-sitter pass)
5. **Resolve usages through import maps** → populate `symbol_refs` (new)

### Import-graph narrowing

`kdb refs -s` doesn't need to scan every file. It only needs files that import from the target file. `code_imports` already has this data — invert the graph.

1. From `code_imports`, build reverse map: `file → Vec<importing_files>`
2. On `kdb refs src/root.rs -s find_root`:
   - Look up reverse imports of `src/root.rs` → `[src/cmd.rs, tests/root.rs, ...]`
   - Only search those files' identifier usages for `find_root`

For a 10k-file monorepo where `root.rs` is imported by 15 files, this scans 15 files instead of 10k.

### What we won't handle (v1)

- **Wildcard imports**: `use crate::*` — can't know which names are pulled in
- **Dynamic references**: `getattr(obj, "handle")`, `obj[methodName]`
- **Trait method dispatch**: which `handle()` is called on a trait object
- **Type-based disambiguation**: two `handle()` functions with different parameter types

These require a full type checker. Import-resolved references cover 95%+ of real-world agent navigation.

### Sub-issue: Correctness evaluation (iss-0028.1)

Test `refs -s` against real repos (zed, tokio, etc.). Compare results with rg and IDE find-references. Quantify recall gaps from re-exports, wildcards, and other known blind spots. Not critical — agents can grep/build as fallback — but want best first-shot coverage.

### v2: Context lines (`-c`)

Add `-c`/`--context <N>` flag (like `grep -C`) to show N lines of surrounding context for each reference. Read the source file and print N lines before/after each match line, with the match line indicated. Applies to text output mode only (JSON already has `snippet`; could add a `context_lines` array).

### v2: Cross-file re-export following

- **Re-exports**: `pub use other::Foo` (Rust), `export { Foo } from './foo'` (TS/JS barrel files), `__all__` re-exports (Python)
- Extremely common in TS/JS codebases (barrel `index.ts` files) and Rust (`pub use` in `mod.rs`)
- Requires multi-hop resolution: if A imports from B, and B re-exports from C, a reference in A should resolve to C's declaration
- Implementation: walk re-export chains during reference resolution, capped at a reasonable depth (e.g. 5 hops)
- High priority — users will hit this quickly in real codebases

### Open questions

- **What counts as a reference?** Start with: calls, type references, field/method access through imported names. Skip: doc comments, string literals, decorators (revisit later).
- **Lazy vs eager indexing?** Should `CodeIndex::build()` always compute `symbol_refs`, or add a separate `CodeIndex::build_with_refs()` for commands that need it? Given that only `refs -s` needs it, lazy is likely better.
- **Transitive imports?** v1: direct importers only. Re-export chains are a future enhancement.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `-s`/`--symbol` flag to `Refs` subcommand |
| `src/cmd.rs` | Branch on `-s` in `refs()` — use `ProjectIndex` when `-s` is present |
| `src/index/mod.rs` | Add `code_symbols`, `symbol_refs` fields to `CodeIndex`; add `SymbolKey`, `SymbolRef` structs |
| `src/index/refs.rs` | Add `collect_symbol_refs()` query function |
| `src/index/code_refs.rs` (new) | Identifier usage extraction + reference resolution logic |
| `src/symbols/extract/*.rs` | May need to expose identifier usage extraction alongside declarations |
| `tests/cli.rs` | Integration tests for `kdb refs -s` |
| `tests/index.rs` | Unit tests for symbol ref resolution |

## Dependencies

All satisfied:

- ~~iss-0024~~ (symbol extraction) — done, `symbols/extract/`
- ~~iss-0026~~ (import resolution) — done, `resolve/`
- ~~iss-0048~~ (CodeIndex split) — done, `CodeIndex` + `ProjectIndex`
