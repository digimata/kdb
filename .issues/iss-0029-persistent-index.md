---
id: 29
title: Persistent disk-backed index
status: proposed
priority: medium
labels:
  - perf
  - index
---

# ISS-0029 :: Persistent disk-backed index

## Intent

The full index build is too slow for large projects on every CLI invocation. Persist per-file index facts to `.kdb/index.bin` and incrementally update.

## What's stored

Per-file facts (symbols, imports, references) keyed by `(rel_path, mtime, size)`. Cross-reference maps (`symbol_refs`, `file_inbound`, etc.) are rebuilt from cached per-file facts on load — cheap since it's just hashmap insertion, no parsing.

## Incremental update flow

1. Load `.kdb/index.bin` (deserialize cached per-file facts)
2. Walk project directory, compare `(mtime, size)` per file against cache
3. **Match** → use cached facts, skip parse
4. **Mismatch** → re-parse with tree-sitter, update cached entry
5. **New file** → parse, add entry. **Deleted file** → remove entry.
6. Rebuild cross-reference maps from (mostly cached) per-file data
7. Write updated index back to disk

## Format

`bincode` or `postcard` via serde. Version header so format changes just trigger a full rebuild. The file is a cache — safe to delete at any time.

## Staleness detection

`(mtime, size)` as fast path (same as `make`/`cargo`). Content hash (xxhash) as fallback when mtime is unreliable (e.g. `git checkout`). Only hash files where mtime changed but size didn't.

## Performance

| Project size | Cold build | Warm (no changes) | Warm (1 file changed) |
|---|---|---|---|
| 30 files | ~100ms | ~20ms | ~25ms |
| 1,000 files | ~3s | ~100ms | ~105ms |
| 10,000 files | ~30s | ~500ms | ~505ms |

## LSP integration

LSP loads the persistent index on startup (instant warm start instead of full rebuild), uses in-memory index with incremental updates at runtime, writes back to disk periodically or on shutdown. CLI and LSP share the same cache file — the LSP warms it while the editor is open, CLI commands benefit.

## Flags

`--fresh` to force a full rebuild (debugging, CI). Missing or corrupt cache file falls back to full build silently.

## Dependencies

- **iss-0028** (code symbol refs): the index that gets persisted

## Changes

| File | Change |
|---|---|
| `src/index/cache.rs` (new) | Serialization, deserialization, staleness detection, incremental update |
| `src/index/mod.rs` | Wire cache into `VaultIndex::build()` |
| `src/main.rs` | Add `--fresh` flag |
