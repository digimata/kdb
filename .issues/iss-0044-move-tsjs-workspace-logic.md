---
id: 44
title: Move TS/JS workspace logic out of resolve/mod.rs
status: done
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
- Introduce `TsjsResolver` struct to own resolver + workspace context (replaces free `resolve()` + `build_resolver()`)
- Replace regex-based import parsing with tree-sitter (`tree-sitter-typescript` / `tree-sitter-javascript` already in deps). Current regexes miss edge cases (multi-line imports, imports inside comments/strings). Tree-sitter gives exact AST nodes for `import_statement`, `export_statement`, and `require` call expressions.

## Notes

- In the iss-0046 target structure, these TS/JS workspace helpers ultimately belong under code indexing (e.g. `index/code/workspace.rs` or a `index/code/tsjs/*` module).
- The `TsjsResolver` struct parallels the `build()` + `resolve()` pattern on `GoWorkspaceCache`, `PythonWorkspaceCache`, and `RustWorkspaceCache` from iss-0043.

## Why

Separation of concerns. The shared `mod.rs` should be language-agnostic. Makes it easier to understand and modify individual language resolvers independently.
