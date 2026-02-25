---
id: 40
title: Extract CmdContext in cmd.rs
status: proposed
priority: high
labels:
  - refactor
---

# ISS-0040 :: Extract CmdContext in cmd.rs

## Intent

7 of 9 command functions repeat the same 5-line setup: resolve start path → `find_root()` → `load_index_ignores()` → optionally `build_with_ignores()`. The `strip_prefix` + `normalize_rel_path` chain is also copy-pasted in `outline()` and `symbols()`.

## Scope

- Create `CmdContext` as a thin CLI-facing wrapper over `project::ProjectContext` (see iss-0047)
- `CmdContext::from_path(path: Option<&PathBuf>) -> Result<Self>` — shared setup
- `CmdContext::build_index(&self) -> Result<VaultIndex>` (or `ProjectIndex` after iss-0048)
- `CmdContext::rel_path(&self, abs: &Path) -> Result<PathBuf>` — the strip + normalize chain
- Refactor `check`, `outline`, `tree`, `symbols`, `refs`, `deps`, `fmt` to use it
- Resolve the `fmt` name collision (`crate::fmt` module vs `pub fn fmt()`)

## Depends on

- iss-0047 (ProjectContext + shared project utilities)

## Why

Eliminates ~35 lines of duplicated setup code across commands. Makes adding new commands cheaper. Satisfies CC-3.3 (type-driven design).
