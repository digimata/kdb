---
id: 50
title: "Configurable ignore patterns via .kdb/ignore"
status: done
priority: medium
labels:
  - enhancement
---

# ISS-0050 :: Configurable ignore patterns via .kdb/ignore

## Problem

`ALWAYS_IGNORED_DIRS` is hardcoded in `src/project/ignore.rs`:

```rust
pub const ALWAYS_IGNORED_DIRS: &[&str] = &[
    ".kdb", ".git", "target", "node_modules", "dist",
    "build", ".next", ".cache", "vendor", "__pycache__", ".venv",
];
```

This causes false negatives on projects where these names are meaningful directories — e.g. kubernetes has a `vendor/` and `build/` that users may want to see in `kdb tree` or index.

Users have no way to override these patterns short of editing the source.

## Proposal

Replace the hardcoded list with a default `.kdb/ignore` file (gitignore syntax) created by `kdb init`. Users can edit it freely.

### `.kdb/ignore` (default contents after `kdb init`)

```gitignore
# Default ignore patterns — edit to suit your project.
.git
target
node_modules
dist
build
.next
.cache
vendor
__pycache__
.venv
```

### Behavior

- `.kdb` itself is always ignored (hardcoded, not user-configurable)
- On `kdb init`, write the default `.kdb/ignore` file
- On project discovery, read `.kdb/ignore` and merge with any `[index] ignore` patterns from `config.toml`
- All commands (`tree`, `check`, `refs`, `deps`, `symbols`, `fmt`, `index`) respect the merged ignore set
- If `.kdb/ignore` is missing, fall back to the current hardcoded list for backwards compatibility

### Migration

Existing projects without `.kdb/ignore` keep current behavior. Users can run `kdb init` in an already-initialized project to generate the file (or create it manually).

## Changes

| File | Change |
|---|---|
| `src/project/ignore.rs` | Read `.kdb/ignore` file, parse as gitignore patterns, merge with config patterns |
| `src/project/mod.rs` | Thread ignore file patterns through `ProjectContext` |
| `src/cmd.rs` | `init()` writes default `.kdb/ignore` alongside `config.toml` |
| `src/tree.rs` | Replace `ALWAYS_IGNORED_DIRS` usage with project ignore set |
| `src/project/ignore.rs` | Keep `.kdb` as sole hardcoded entry; remove rest from `ALWAYS_IGNORED_DIRS` |
