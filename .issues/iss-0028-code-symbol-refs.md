---
id: 28
title: Code symbol references (refs -s)
status: proposed
priority: high
labels:
  - feat
  - refs
---

# ISS-0028 :: Code symbol references (`refs -s`)

## Intent

Find all references to a code symbol across the project using import-resolved precision. The core indexing architecture that extends VaultIndex with code symbol awareness.

## CLI

```
kdb refs <file>                       # markdown inbound refs (current)
kdb refs <file>#<heading>             # markdown heading refs (current)
kdb refs <file> -s <name>             # code symbol references (new)
kdb refs <file> -s <name> --count     # just the count
```

Example:

```
$ kdb refs src/root.rs -s find_root
src/root.rs:34:8            pub fn find_root(start: &Path) -> Result<PathBuf> {
src/cmd.rs:74:18            root::find_root(&start)
src/cmd.rs:92:18            root::find_root(&file_abs)
src/cmd.rs:147:18           root::find_root(&file_abs)
src/fmt/mod.rs:42:18        root::find_root(&start)
tests/root.rs:15:10         find_root(&dir)
```

Definition site is included by default (like Zed's `include_declaration: true`).

## Architecture

### Why not text search?

Text search (grep-style) gives false positives — it can't distinguish identifiers from strings, comments, or identically-named symbols in unrelated scopes. If `kdb refs -s` is just grep with nicer output, there's no reason to build it.

### Why not parse on every query?

Parsing every file with tree-sitter on each `kdb refs -s` invocation works but throws away the work. For a 10k-file monorepo at ~1-5ms per parse, that's 10-50 seconds per query with no caching.

### Why not delegate to existing language servers?

We'd need to spawn and manage rust-analyzer, tsserver, pyright, and gopls — four separate servers with 5-30 second cold start times each, each with its own project configuration. That's the opposite of kdb's design as a single lightweight binary. And the agent can already use these through the editor's built-in find-all-references if the LSP is running.

### Chosen approach: unified index

Index code symbols and import-resolved references into VaultIndex at build time — the same architecture we already use for markdown headings and links. The index is language-indifferent and vault-wide — all languages stored homogeneously.

```
VaultIndex
├── files: HashMap<PathBuf, FileEntry>              # markdown (existing)
│   ├── headings, links
│
├── code_files: HashMap<PathBuf, CodeFileEntry>      # code (new)
│   ├── symbols: Vec<Symbol>                         # definitions (fn, struct, class, etc.)
│   ├── imports: Vec<ResolvedImport>                 # import statements, resolved to file paths
│   └── references: Vec<ResolvedReference>           # identifier usages, resolved via import map
│
├── file_inbound: HashMap<PathBuf, Vec<LinkRef>>     # markdown refs (existing)
├── heading_inbound: HashMap<HeadingKey, Vec<LinkRef>>
│
└── symbol_refs: HashMap<SymbolKey, Vec<SymbolRef>>  # code refs (new)
```

Suggested key/row shapes (for clarity, not a hard API):

- `SymbolKey` should uniquely identify a declaration within an index build.
  - Minimal: `(def_file_rel_path, language, name, parent, kind, start_byte)`.
  - `start_byte` (or `line`) is the tie-breaker for overloaded/duplicated names.
- `SymbolRef` is one reference row. It should carry enough to print the stable text format.
  - `(source_file_rel_path, line, col, snippet, is_definition)`.
  - `is_definition` supports including the declaration site by default.

### Build pipeline

1. Walk project, discover all files (existing)
2. Parse `.md` files → headings + links (existing)
3. Parse code files with tree-sitter → symbols + imports + identifier usages (new)
4. Resolve imports → build per-file import map (new, uses iss-0026 resolution)
5. Resolve identifier usages through import maps → populate `symbol_refs` (new)
6. Build cross-reference maps (extend existing)

Every query command is a view over this index:
- `kdb symbols <file>` → `code_files[file].symbols`
- `kdb symbols <file> -s name` → find symbol, slice source at its span
- `kdb refs <file> -s name` → `symbol_refs[key]`
- `kdb deps <file>` → `code_files[file].imports` (code) or `files[file].links` (markdown)
- `kdb codemap` → iterate all entries
- LSP → same index, cached and updated incrementally on file changes

### Import-resolved references (the key precision mechanism)

The difference between "smart grep" and actual reference finding is import resolution (iss-0026). When indexing a file:

1. Parse its import/use statements with tree-sitter
2. Build a per-file import map: `HashMap<String, ResolvedPath>` (via iss-0026 resolvers)
3. When we encounter an identifier in the AST, check the import map
4. If it resolves → store as a confirmed reference to that specific symbol
5. If it doesn't resolve (local variable, built-in, etc.) → don't store

Example:

```rust
// src/api/auth.rs — defines pub fn handle()
// src/api/users.rs — also defines pub fn handle()

// src/router.rs
use crate::api::auth;
auth::handle(req)    // ← resolves to auth.rs::handle via import map ✓

// src/api/users.rs
handle(input)        // ← resolves to self::handle (local), NOT auth::handle ✗
```

### Import-graph narrowing for refs

`kdb refs -s` doesn't need to scan every file in the project. It only needs files that *import from* the target file. The import graph (a byproduct of the index build) tells us exactly which files those are.

**Flow:**

1. Build the import graph: for each file, record which files it imports from → invert to get `file → Vec<importing_files>`
2. On `kdb refs src/root.rs -s find_root`:
   - Look up reverse imports of `src/root.rs` → `[src/cmd.rs, src/fmt/mod.rs, tests/root.rs]`
   - Only scan those files for `find_root` identifier references
   - Skip the other 9,985 files entirely

**Impact:** For a 10k-file monorepo where `root.rs` is imported by 15 files, refs scans 15 files instead of 10k. ~75ms instead of ~30s.

**Transitive imports:** For v1, only check direct importers. Transitive re-export tracking is a future enhancement.

### What we won't handle (and that's OK)

- **Re-exports**: `pub use other_module::Foo` — following re-export chains requires multi-hop resolution
- **Wildcard imports**: `use crate::api::*` — can't know which names are pulled in without full module analysis
- **Dynamic references**: `getattr(obj, "handle")` in Python, `obj[methodName]` in JS
- **Trait method dispatch**: Rust `impl Trait for Type` — knowing which `handle()` is called on a trait object requires type inference
- **Type-based disambiguation**: two `handle` functions that take different types — requires the type checker

These are language server territory. For agent navigation, import-resolved references cover 95%+ of real-world usage patterns.

### Performance budget

- Tree-sitter parse: ~1-5ms per file
- 30 files (this repo): ~50-150ms — negligible
- 1,000 files (medium project): ~1-5s — acceptable for CLI
- 10,000 files (large monorepo): ~10-50s — needs lazy strategy for CLI; fine for LSP (cached)

Memory note: storing refs can dominate. Avoid allocating paths/strings per row; use file-id interning and keep ref rows compact.

### Open questions

- **What counts as a reference?** Calls only, or also type references, trait bounds, decorators/annotations, macro invocations, import specifiers, re-exports, doc-comment mentions?
- **Cross-language refs:** do we ever attempt cross-language mapping (e.g. TS -> Rust via generated bindings), or keep refs strictly language-local?
- **Scope awareness:** how far past import/use maps do we go? (locals shadowing imports, nested scopes, module/glob imports, re-export chains)
- **Lazy vs full indexing:** should `VaultIndex::build()` always compute `symbol_refs`, or support build profiles (markdown-only, code-symbols-only, full)?

## Dependencies

- **iss-0024** (native symbol display): symbol model with spans
- **iss-0026** (workspace module resolution): import maps for cross-package reference resolution

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `-s`/`--symbol` flag to `Refs` subcommand |
| `src/cmd.rs` | Branch on `-s` presence in `refs()` |
| `src/index/mod.rs` | Extend VaultIndex with `code_files`, `symbol_refs` maps |
| `src/index/code.rs` (new) | Code file indexing — tree-sitter parse, symbol + reference extraction |
| `src/index/refs.rs` | Extend to query `symbol_refs` map for code references |
