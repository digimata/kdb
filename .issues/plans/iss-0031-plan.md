---
title: "Parallel file parsing during index build"
date: 2026-02-25
status: draft
affects: "index build performance"
---

## Context

Both `VaultIndex::build_with_ignores` and `build_workspace_import_index` parse files sequentially in a single loop. Each file is independent — tree-sitter parsers are already allocated fresh per-file, and per-file extraction is stateless (returns a `Vec` with no shared mutable state).

For large projects, parsing dominates build time. At ~3ms per tree-sitter parse on a 10k-file repo, single-threaded build takes ~30s. Spreading across 8 cores cuts that to ~4s.

Rayon is the natural fit: `par_iter` over file paths, collect results, then build cross-reference maps single-threaded afterward.

## Changes

### 1. Add `rayon` dependency

`Cargo.toml` — add `rayon = "1.10"`.

### 2. Parallelize `VaultIndex::build_with_ignores`

`src/index/mod.rs` lines 386–420.

Current sequential loop:
```
discover files → for each file: read + parse_markdown → insert into BTreeMap
```

New parallel version:
```
discover files → par_iter: read + parse_markdown → collect Vec<(PathBuf, FileEntry)>
                → single-threaded: insert into BTreeMap + populate_inbound
```

- Use `rayon::iter::IntoParallelRefIterator` on the discovered paths
- Each thread reads the file and calls `parse_markdown` independently
- Collect into a `Vec<(PathBuf, FileEntry)>` (avoids concurrent map writes)
- Build the `BTreeMap` and call `populate_inbound` single-threaded (cheap)

### 3. Parallelize `build_workspace_import_index`

`src/resolve/mod.rs` lines 101–141.

Same pattern:
```
discover files → par_iter: read + resolve_imports_for_language → collect
              → single-threaded: sort + insert into BTreeMap
```

- `WorkspaceCaches` is built first (single-threaded, fast)
- `WorkspaceCaches` fields are all read-only during the per-file phase, but the resolvers take `&self` — need to verify `WorkspaceCaches` is `Send + Sync` (it's `HashMap`/struct of `HashMap` — should be fine)
- Each thread reads file, resolves imports, sorts, returns `(PathBuf, Vec<ResolvedImport>)`
- Collect and insert into `BTreeMap` after

### 4. No threshold for small projects

The issue mentions a threshold for small projects (<100 files). Rayon's overhead is negligible for small workloads — it just runs on the calling thread. Not worth the complexity.

## Files touched

```
┌──────────────────────────┬──────────────────────────────────────────────┐
│         File             │                   Action                     │
├──────────────────────────┼──────────────────────────────────────────────┤
│ Cargo.toml               │ Edit (add rayon dep)                         │
│ src/index/mod.rs         │ Edit (parallelize build_with_ignores loop)   │
│ src/resolve/mod.rs       │ Edit (parallelize build_workspace_import_index loop) │
└──────────────────────────┴──────────────────────────────────────────────┘
```

## Verification

1. `cargo build` — compiles clean
2. `cargo test` — all existing tests pass
3. `cargo clippy` — zero warnings
4. `kdb check` on this repo — same results as before
5. Manual: `time kdb check` on a larger repo to confirm speedup (optional)
