---
id: 44
title: Move TS/JS workspace logic out of resolve/mod.rs
status: proposed
priority: medium
labels:
  - refactor
---

# ISS-0044 :: Move TS/JS Workspace Logic Out of resolve/mod.rs

## Intent

`resolve/mod.rs` is ~900 lines. Roughly 400 of those (lines 286–897) are TS/JS-specific: `discover_workspace_packages`, `workspace_patterns`, `read_pnpm_workspace_patterns`, `read_package_json_workspace_patterns`, `resolve_workspace_specifier`, `resolve_workspace_entry`, `resolve_package_target`, `export_target`, `first_export_string`.

These functions are only called from `tsjs::resolve` and the workspace package discovery. They don't belong in the shared module.

## Scope

- Move TS/JS workspace functions into `resolve/tsjs.rs` (or `resolve/tsjs/` submodule if it's already large)
- Keep only types, dispatch, and truly shared helpers in `resolve/mod.rs`
- `resolve/mod.rs` should drop from ~900 to ~400 lines

## Notes

- In the iss-0046 target structure, these TS/JS workspace helpers ultimately belong under code indexing (e.g. `index/code/workspace.rs` or a `index/code/tsjs/*` module).

## Why

Separation of concerns. The shared `mod.rs` should be language-agnostic. Makes it easier to understand and modify individual language resolvers independently.
