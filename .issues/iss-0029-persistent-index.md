---
id: 29
title: Persistent disk-backed index
status: done
priority: medium
labels:
  - perf
  - index
depends:
  - 28
---

# ISS-0029 :: Persistent disk-backed index

## Intent

The full index build is too slow for large projects on every CLI invocation. Persist per-file index facts to `.kdb/index.bin` and incrementally update.

Only commands that need the cross-file index pay this cost (`refs`, `deps`, `check`, `graph`). Single-file commands (`symbols <file>`) remain parse-on-demand — they're already fast and don't need the cache.

## Reference design: ruff

The primary reference implementation is [ruff](https://github.com/astral-sh/ruff)'s `crates/ruff/src/cache.rs` + `crates/ruff_cache/`. ruff is a Rust CLI that caches per-file lint/format results to a single bincode blob. Key design points adopted:

- **Single cache blob** — `FxHashMap<RelativePathBuf, FileCache>` serialized with bincode. No one-file-per-source-file on disk (avoids filesystem overhead at scale).
- **mtime-based staleness key** — `FileCacheKey { file_last_modified, file_permissions_mode }` hashed to a `u64` via a `CacheKey` trait (backed by `SeaHasher`). If the key mismatches the cached key, the entry is stale.
- **`last_seen` GC** — each cache entry tracks a `last_seen` timestamp. On write-back, entries not seen for 30 days are pruned. This handles deleted files without an explicit walk-diff step.
- **Atomic write** — `tempfile::NamedTempFile` + `persist()` for crash-safe cache updates.
- **Version in cache path** — `.ruff_cache/<VERSION>/<settings_hash>` so format changes auto-invalidate.
- **Graceful degradation** — corrupt or missing cache silently falls back to full build. The cache is advisory.

### Why not cargo's approach

Cargo uses one fingerprint file per compilation unit with complex dependency-edge tracking. This is wrong for kdb because: (1) kdb needs all per-file facts loaded in memory to build cross-reference maps — cargo never aggregates across units; (2) per-source-file cache files would mean 10k+ filesystem entries at scale; (3) cargo's fingerprint answers "rebuild yes/no" while kdb needs the actual parse artifacts back.

### Why not rkyv / zero-copy

rkyv would eliminate deserialization cost entirely (mmap + pointer cast), but requires parallel `Archived<T>` types for every struct, makes format evolution harder, and adds significant complexity. At kdb's scale (1-10k files, ~2-5MB cache), bincode deserializes in ~5ms. Not worth the complexity for v1. Upgrade path exists if needed.

## What's stored

Per-file facts keyed by `(rel_path, mtime_key)`. We cache **post-resolution** artifacts — both parsed symbols and resolved imports (with `resolved_path` already filled in).

```rust
/// On-disk cache: project root + per-file facts.
#[derive(bincode::Encode, bincode::Decode)]
struct IndexCache {
    /// kdb version that wrote this cache (format changes → full rebuild).
    version: String,
    /// Hash of workspace manifest mtimes (go.mod, Cargo.toml, package.json).
    /// If any manifest changes → full rebuild (import resolution depends on these).
    manifest_key: u64,
    /// Per-file cached parse results.
    files: HashMap<RelativePathBuf, CachedFileFacts>,
}

/// Cached parse artifacts for a single source file.
#[derive(bincode::Encode, bincode::Decode)]
struct CachedFileFacts {
    /// Staleness key: hash of (mtime, size). If mismatch → re-parse.
    key: u64,
    /// Millis since epoch when this entry was last used.
    last_seen: u64,
    /// What kind of file produced these facts.
    kind: CachedFileKind,
}

enum CachedFileKind {
    Markdown {
        headings: Vec<CachedHeading>,
        links: Vec<CachedLink>,
    },
    Code {
        /// Parsed symbol declarations (name, kind, line, parent, visibility).
        symbols: Vec<CachedSymbol>,
        /// Post-resolution imports (raw specifier, resolved_path, kind, names).
        imports: Vec<CachedImport>,
    },
}
```

### Why post-resolution imports

The current pipeline has two expensive passes per code file:

1. **`build_workspace_import_index()`** — parse + resolve imports (language resolvers do filesystem stat calls to find target files like `./foo/bar.ts` or `./foo/bar/index.ts`)
2. **`SymbolIndex::build()`** — re-read + re-parse every file for symbols, reexports, usage scanning

Caching post-resolution `ResolvedImport`s (with `resolved_path` filled in) lets us skip both the tree-sitter parse *and* the resolver filesystem probing for cached files. The alternative — caching raw imports and re-resolving every time — would still require running the language resolvers (expensive at 10k+ files).

### Manifest-keyed invalidation

Import resolution depends on workspace context (`go.mod`, `Cargo.toml`, `package.json`, `tsconfig.json`). These change rarely. We hash their mtimes into `manifest_key` — if any manifest changes, the entire cache is discarded and rebuilt. This is simple, correct, and avoids per-file cross-invalidation complexity.

### Known correctness gap

If file A moves/renames but file B (which imports A) is unchanged, B's cached `resolved_path` is stale. This self-corrects when B is next edited, and `--fresh` always works. Same tradeoff ruff makes with mtime-based caching.

### What's NOT cached

Cross-reference maps (`symbol_refs`, `file_inbound`, `heading_inbound`) are rebuilt from per-file facts on every load. This is cheap (hashmap insertion + usage scanning, no tree-sitter parsing) and avoids the cross-file invalidation problem entirely. The `SymbolIndex::build()` pipeline (build_symbol_lookup → build_reexport_lookup → build_module_scopes → resolution_loop → seed_definition_refs → link_usage_refs) runs fresh each time, but operates on cached symbols and imports rather than re-reading files from disk.

**Note**: the usage scanning step (`link_usage_refs`) currently re-reads source files to scan for identifier usages via tree-sitter. This is the one part that can't be skipped by caching symbols+imports alone — it needs the actual source text. Options: (a) accept this cost for v1 (it's parallelized and faster than full parsing), (b) cache usage scan results per-file as a future optimization.

## Incremental update flow

1. Load `.kdb/index.bin` → deserialize `IndexCache`
2. Check `manifest_key` — if workspace manifests changed → discard cache, full rebuild
3. Walk project directory, compute `(mtime, size)` per file
4. For each file:
   - **Cache hit** (key matches) → use cached facts, update `last_seen`
   - **Cache miss** (key differs or new file) → parse with tree-sitter, resolve imports, store new entry
5. GC: prune entries with `last_seen` older than 30 days (handles deleted files)
6. Rebuild cross-reference maps from (mostly cached) per-file facts
7. Run the requested command (`refs`, `deps`, etc.)
8. Write updated cache to disk (atomic via tempfile + rename)

## Staleness detection

`(mtime, size)` hashed to a `u64` via `SeaHasher`, same as ruff. This is the right tradeoff:

- **Fast**: one `stat()` call per file, no reads
- **Reliable enough**: covers all normal edits. The known failure mode (git checkout preserving size but changing content with same mtime) is rare in practice.
- **No content hashing needed for v1**: ruff ships with mtime-only and it works. If we hit edge cases later, we can add content hash as a fallback for same-mtime-same-size files.

## Scope: which commands use the cache

| Command | Needs full index? | Uses cache? |
|---|---|---|
| `refs <file> -s <sym>` | Yes (cross-file) | **Yes** |
| `deps <file>` | Yes (import resolution) | **Yes** |
| `check` | Yes (link validation) | **Yes** |
| `graph` | Yes (dependency graph) | **Yes** |
| `symbols <file>` | No (single file) | **No** — parse on demand |
| `tree` | No (filesystem only) | **No** |
| `fmt` | No (single file) | **No** |
| `init` | No | **No** |

## Format

`bincode` via `bincode::encode_to_vec` / `bincode::decode_from_reader`. Chosen because:

- ruff validates it at scale (production use across millions of projects)
- Zero-friction serde derives (`#[derive(bincode::Encode, bincode::Decode)]`)
- Fast enough (~5ms deserialize for ~5MB cache)
- Well-maintained, stable format

Version string embedded in the cache struct. Mismatch → discard and full rebuild.

## Performance targets

Baseline data (cold, v0.10.2):

| Repo | Files | Cold |
|---|---|---|
| mio (Rust) | ~200 | 0.05s |
| poetry (Python) | ~400 | 0.26s |
| tokio (Rust) | ~766 | 0.32s |
| airstore (Go) | ~312 | 0.16s |
| kubernetes (Go) | ~16k | 9.0s |

## LSP integration

LSP loads the persistent index on startup (instant warm start instead of full rebuild), uses in-memory index with incremental updates at runtime, writes back to disk periodically or on shutdown. CLI and LSP share the same cache file — the LSP warms it while the editor is open, CLI commands benefit.

## Flags

`--fresh` to force a full rebuild (debugging, CI). Missing or corrupt cache file falls back to full build silently.

## Future: Salsa for LSP-grade incrementality

The v1 design (mtime + bincode) is right for the CLI: rebuild cross-reference maps from cached per-file facts on every invocation. This is fast enough — step 5 is just hashmap insertion.

For the LSP, a more granular option is the [Salsa](https://github.com/salsa-rs/salsa) incremental computation framework (used by rust-analyzer and ruff's ty type checker). Salsa tracks query dependencies automatically — when a file changes, only the queries that transitively depend on that file are re-run. This would reduce LSP update latency from "rebuild all cross-ref maps" (~100ms at 1k files) to "re-resolve affected imports" (~5ms).

Adopting Salsa would require structuring resolution as pure query functions with explicit inputs/outputs. The migration path: implement v1 with bincode, organize resolution as Salsa-compatible pure functions, add Salsa later if LSP latency becomes a problem.

## Future: rkyv upgrade path

If cache deserialization becomes a bottleneck (50k+ file projects), swap bincode for rkyv zero-copy deserialization. rkyv deserializes via mmap + pointer cast (~0ms) but requires `#[derive(Archive)]` on all cached types and makes format evolution harder. Only worth it if profiling shows bincode deser as a real bottleneck.

## Benchmarking harness (from iss-0034)

Establish a benchmark suite to validate the persistent index and catch regressions. Subsumes iss-0034.

- **`--timings` flag**: print per-phase breakdown (walk, parse, resolve, query) to stderr
- **Criterion benchmarks**: `benches/` with criterion for hot paths (tree-sitter parse, index build, import resolution)
- **CI regression check**: track benchmark results across commits

## Dependencies

- **iss-0028** (code symbol refs): the index that gets persisted

## Changes

| File | Change |
|---|---|
| `src/index/cache.rs` (new) | `IndexCache` struct, bincode ser/de, staleness detection, `last_seen` GC, atomic write |
| `src/index/mod.rs` | Wire cache into `ProjectIndex::build()` — load cache, diff against walk, re-parse stale files, rebuild cross-refs, write back |
| `src/cmd.rs` | Plumb cache through `build_project_index()` for `refs`/`deps`/`check`/`graph`; add `--fresh` flag |
| `src/main.rs` | Add `--fresh` and `--timings` CLI flags |
| `Cargo.toml` | Add `bincode`, `seahash`, `tempfile` deps |
| `benches/` (new) | Criterion benchmarks for parse, index build, resolution |
