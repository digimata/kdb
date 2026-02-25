---
id: 47
title: ProjectContext + Shared Project Utilities
status: done
priority: high
labels:
  - refactor
---

# ISS-0047 :: ProjectContext + Shared Project Utilities

## Intent

Make shared `project semantics` explicit and canonical:

- root discovery
- config loading
- ignore compilation
- safe relative-path normalization
- filesystem discovery/walking
- shared text/offset utilities used by LSP + indexing

Today these concerns are split across `src/root.rs`, `src/config.rs`, `src/discovery.rs`, `src/index/mod.rs`, `src/resolve/mod.rs`, `src/tree.rs`, and `src/lsp/backend.rs` with duplicated helpers and slightly different behavior.

## Scope

- Create `src/project/` module:
  - `src/project/root.rs` (move from `src/root.rs`, keep re-export during transition)
  - `src/project/config.rs` (move from `src/config.rs`, keep re-export during transition)
  - `src/project/paths.rs` canonical `normalize_rel_path` and safe `rel_path_from_root`
  - `src/project/ignore.rs` canonical ignore globset building + `always ignored dirs`
  - `src/project/discover.rs` single walker API used by markdown indexing, code indexing, and `kdb tree`
  - `src/project/text.rs` line-start/offset helpers used by LSP + code resolution
- Introduce `ProjectContext` (in `src/project/mod.rs`) that owns:
  - canonical root
  - loaded ignore patterns
  - compiled ignore matchers
  - shared discovery helpers
- Migrate call sites to use these shared modules:
  - CLI (via iss-0040 CmdContext)
  - LSP backend
  - index/vault and code indexing
  - tree building

## Why

- Eliminates drift and duplicated logic.
- Ensures consistent ignore + discovery semantics across commands.
- Enables type-driven design: pass `&ProjectContext` instead of threading (root, ignores, globsets, etc.) everywhere.

## Non-goals

- No behavior change required to land the module split (prefer compatibility re-exports).
- Does not require splitting `VaultIndex` yet (see iss-0048).
