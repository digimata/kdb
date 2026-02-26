---
title: "Configurable ignore patterns via .kdb/ignore"
date: 2026-02-26
status: done
affects: "ignore handling, init, tree, discover, resolve"
---

## Context

`ALWAYS_IGNORED_DIRS` is a hardcoded `&[&str]` constant in `src/project/ignore.rs` containing 11 directory names. It's consumed in 6 call sites:

1. `src/tree.rs:320` — `is_ignored_path()` checks dir names directly
2. `src/project/discover.rs:108` — `should_visit_entry()` via `ignored_dirs` param
3. `src/fmt/mod.rs:450` — passed to `discover_files()`
4. `src/index/cache.rs:280` — passed to `discover_files()`
5. `src/index/mod.rs:799` — passed to `discover_files()`
6. `src/resolve/{mod,rust,tsjs,python}.rs` — 4 inline walker filters

Additionally, `config.toml`'s `[index].ignore` patterns are loaded separately via `config::load_index_ignores()` and compiled into a `GlobSet` on `ProjectContext`. These two ignore mechanisms are independent — the hardcoded list is a name-based dir check, while config patterns go through globset matching.

The goal: replace the hardcoded list (except `.kdb`) with a user-editable `.kdb/ignore` file. Parse it as gitignore-style patterns, merge with config patterns, and thread through all call sites.

## Changes

### 1. Add `.kdb/ignore` file reader (`src/project/ignore.rs`)

- New function `load_ignore_file(root: &Path) -> Result<Vec<String>>` that reads `.kdb/ignore`, strips comments/blanks, returns patterns.
- If the file doesn't exist, return the current hardcoded defaults (minus `.kdb`) for backwards compat.
- Keep `.kdb` as the sole hardcoded constant (`ALWAYS_IGNORED_DIR: &str = ".kdb"`).
- Remove `ALWAYS_IGNORED_DIRS` array.

### 2. Merge ignore sources in `ProjectContext::from_root()` (`src/project/mod.rs`)

- Call `load_ignore_file()` to get file-based patterns.
- Concatenate with `load_index_ignores()` config patterns.
- Compile the merged set into `ignore_set`.
- Drop the separate `ignore_patterns` field (or keep it for debug — TBD, probably keep).

### 3. Write default `.kdb/ignore` on `kdb init` (`src/cmd.rs`)

- After creating `.kdb/` and `config.toml`, also write `.kdb/ignore` with the default patterns from the issue.
- If re-running init on an existing project to generate the file, we'd need to relax the "already exists" bail — but the issue says "users can create it manually" so we keep init strict for now.

### 4. Remove `ignored_dirs` param from `discover_files()` (`src/project/discover.rs`)

- The `ignored_dirs: &[&str]` parameter becomes unnecessary since all patterns are now in the `GlobSet`.
- BUT: the current approach checks dir *names* (basename only), while globset checks *paths*. A pattern `target` in globset won't match `foo/target/` unless written as `**/target` or `target/**`.
- Decision: `load_ignore_file()` will auto-wrap bare names as `**/NAME` patterns so they match at any depth (matching current behavior). The `.kdb/ignore` file will contain bare names for readability; the loader adds the glob wrapping.
- Remove `ignored_dirs` param; `should_visit_entry()` only checks `ignore_set`.
- Update all 3 callers: `fmt/mod.rs`, `index/cache.rs`, `index/mod.rs`.

### 5. Update `tree.rs` (`is_ignored_path`)

- Remove the `ALWAYS_IGNORED_DIRS.contains()` check — it's now in the globset.
- The `.kdb` dir still needs a hardcoded check (or it's already in the globset from the loader's always-include).
- Actually: the loader always includes `.kdb` patterns regardless of file contents. So the tree function just uses the globset.

### 6. Update resolve walkers (`src/resolve/{mod,rust,tsjs,python}.rs`)

- These 4 files have inline `WalkBuilder` filters that check `ALWAYS_IGNORED_DIRS`.
- They need access to the ignore set. Currently, resolve functions receive `root: &Path` but not the project context.
- Options:
  - (a) Pass `&GlobSet` or `&[String]` to the resolve functions — minimal change
  - (b) Pass `&ProjectContext` — cleaner but larger signature change
  - (c) Build a standalone ignore set in each resolver from the root path
- Going with **(a)**: thread `&GlobSet` into the 4 walker sites. The callers (`refs` command, index builder) already have `ProjectContext`, so they just pass `&ctx.ignore_set`.

## Files touched

```
┌────────────────────────────┬─────────────────────────────────────────────────┐
│         File               │          Action                                 │
├────────────────────────────┼─────────────────────────────────────────────────┤
│ src/project/ignore.rs      │ Edit: add load_ignore_file(), remove constant   │
│ src/project/mod.rs         │ Edit: merge ignore sources in from_root()       │
│ src/project/discover.rs    │ Edit: remove ignored_dirs param                 │
│ src/cmd.rs                 │ Edit: write .kdb/ignore on init                 │
│ src/tree.rs                │ Edit: remove ALWAYS_IGNORED_DIRS check          │
│ src/resolve/mod.rs         │ Edit: thread ignore_set, remove re-export       │
│ src/resolve/rust.rs        │ Edit: use ignore_set instead of constant        │
│ src/resolve/tsjs.rs        │ Edit: use ignore_set instead of constant        │
│ src/resolve/python.rs      │ Edit: use ignore_set instead of constant        │
│ src/fmt/mod.rs             │ Edit: drop ignored_dirs arg to discover_files   │
│ src/index/cache.rs         │ Edit: drop ignored_dirs arg to discover_files   │
│ src/index/mod.rs           │ Edit: drop ignored_dirs arg to discover_files   │
└────────────────────────────┴─────────────────────────────────────────────────┘
```

## Verification

1. `cargo build` — zero warnings
2. `cargo test` — all existing tests pass
3. Manual: `kdb init` on a fresh dir → `.kdb/ignore` created with defaults
4. Manual: `kdb tree` on kdb repo — same output as before (no regression)
5. Manual: edit `.kdb/ignore` to remove `target`, run `kdb tree` — `target/` now visible
6. Manual: `kdb refs` / `kdb check` — still respect ignore patterns
7. `cargo clippy` — zero warnings
