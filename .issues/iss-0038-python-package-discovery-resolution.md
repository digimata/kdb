---
id: 38
title: Python package discovery resolution
status: proposed
priority: high
labels:
  - feat
  - deps
  - refs
  - python
---

# ISS-0038 :: Python package discovery resolution

## Intent

Add manifest-aware Python import resolution so absolute package imports resolve to local files.

Current state from iss-0026:
- Relative Python imports resolve
- Manifest/package discovery (`pyproject.toml` / `setup.py`) is missing

## Scope

1. Discover local Python packages from manifests
   - `pyproject.toml` package layouts (`src/` and flat layouts)
   - fallback support for common `setup.py` patterns
2. Build package map
   - import package/module name -> local directory/file
3. Resolve absolute imports to local files
   - `import mypkg.utils`
   - `from mypkg.subpkg import thing`

## Out of scope

- Full Python environment/site-packages resolution
- Dependency manager lockfile semantics (poetry/pdm/uv)

## Done when

- `kdb deps` resolves absolute imports for local package modules in `pyproject.toml` fixtures.
- Both `src/` layout and flat-layout package fixtures resolve correctly.
- Relative import behavior remains unchanged.
- Non-local packages remain external/unresolved.
- Tests cover:
  - `import pkg.mod`
  - `from pkg.sub import name`
  - `src/` layout package mapping

## Expected changes

| File | Change |
|---|---|
| `src/resolve/python.rs` | Add manifest-aware package discovery + absolute import resolution |
| `src/resolve/mod.rs` | Add/extend workspace cache structures for Python package maps |
| `tests/cli.rs` | Add Python package-layout deps tests |
