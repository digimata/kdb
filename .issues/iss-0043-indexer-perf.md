---
id: 43
title: "Indexer performance: parse once + parallelize"
status: done
priority: medium
labels:
  - perf
  - index
---

# ISS-0043 :: Indexer performance: parse once + parallelize

Two independent wins that reduce cold-build latency without a persistent
index (0029).

## 1. Parse once, reuse tree

Every code file is parsed by tree-sitter 3-4 times per `refs -s` invocation:

| Step | Where | Parse call |
|---|---|---|
| Import collection | `RustImportCollector::parse_tree()`, `TsjsResolver::parse_tree()`, etc. | 1 |
| Re-export extraction | `collect_reexports()` → `parse_tree()` | 2 |
| Symbol extraction | `extract_symbols()` → `parse_tree()` | 3 |
| Usage scanning | `UsageScanner::collect()` → `parse_tree()` | 4 |

Tree-sitter is fast (~1ms per file) but it's the dominant CPU cost.
Parsing once and threading the tree through the pipeline would cut
parse time by ~70%.

### Approach

Add a `parsed_tree: tree_sitter::Tree` field to `CodeFileFacts`. Parse
during `load_code_files()` (or lazily on first access). Pass the tree
to `extract_symbols()`, `extract_reexport_bindings()`, and
`UsageScanner::collect()` instead of letting each re-parse from source.

The import resolvers run earlier (in `build_workspace_import_index()`
before the `Indexer` exists), so they'd keep their own parse. That's
fine — it's one of the four, and refactoring the resolver pipeline to
share trees would be a bigger change for diminishing returns.

### Changes

| File | Change |
|---|---|
| `src/index/code.rs` | Add `tree` to `CodeFileFacts`, parse in `load_code_files()` |
| `src/symbols/mod.rs` | `extract_symbols()` takes `&Tree` instead of parsing |
| `src/resolve/mod.rs` | `extract_reexport_bindings()` takes `&Tree` instead of parsing |
| `src/index/code.rs` | `UsageScanner::collect()` takes `&Tree` instead of parsing |

## 2. Parallelize the indexer

`Indexer::build()` is fully sequential. The per-file steps (symbol
extraction, usage scanning) are independent across files and safe to
parallelize with rayon.

Import resolution already uses `par_iter()` in
`build_workspace_import_index()`. The indexer should do the same.

### Approach

- `extract_symbols()`: `par_iter()` over `code_files`, collect results
- `link_usage_refs()`: `par_iter()` over files, collect usage rows, then
  merge into `symbol_refs` (requires a merge step since `symbol_refs` is
  shared state)
- `link_go_same_package_refs()`: same pattern — parallel scan per file,
  sequential merge

The merge step adds a small amount of code but the per-file work
(tree-sitter walk + identifier matching) dominates.

### Changes

| File | Change |
|---|---|
| `src/index/code.rs` | `extract_symbols()` uses `par_iter()` |
| `src/index/code.rs` | `link_usage_refs()` collects in parallel, merges sequentially |
| `Cargo.toml` | rayon already a dependency |

## Priority

Not urgent. Current performance is fine for projects under ~5k files.
These become relevant for large monorepos where cold builds take >5s.
Persistent index (0029) is the bigger win for repeat invocations; these
help with the cold path.
