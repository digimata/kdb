---
id: 43
title: Consolidate resolve workspace caches
status: done
priority: medium
labels:
  - refactor
---

# ISS-0043 :: Consolidate Resolve Workspace Caches

## Intent

`resolve_imports_for_language()` takes 8 arguments — root, source_file, source, language, plus four separate workspace caches (TS/JS, Rust, Go, Python). Each language only uses one cache; the other three are ignored.

## Scope

- Create `WorkspaceCaches` struct holding all four cache types
- `WorkspaceCaches::build(root, ignore_patterns) -> Result<Self>` — calls all `build_workspace_cache` functions
- Simplify `resolve_imports_for_language` to take `&WorkspaceCaches` instead of 4 separate params
- Update `build_workspace_import_index` and any callers

## Notes

- In the iss-0046 target structure this cache lives with code indexing (not as a CLI-threaded parameter soup).

## Why

Eliminates the 8-argument function signature. Makes adding new language resolvers cheaper (just add a field to the struct). Satisfies CC-3.3.

## Depends on

- None (iss-0041 is already resolved/archived)
