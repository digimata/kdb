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

## Implementation plan

### Context

- Python resolver currently supports relative imports but not package discovery for absolute imports.
- `pyproject.toml` is the primary source of package layout truth and has Rust parser support.
- `setup.py` is executable Python, so support must be best-effort rather than fully general.

### Changes

1. Add Python package cache model
   - File: `src/resolve/mod.rs`
   - Add cached map of Python package/module prefixes to local roots, built once per index build.
   - Decision: keep cache independent from resolver internals so `refs -s` can reuse it later.

2. Parse `pyproject.toml` for package roots
   - File: `src/resolve/python.rs`
   - Use `pyproject-toml` (or equivalent strongly typed parser) to detect `src/` and flat layouts and derive package roots.
   - Decision: prefer typed parsing over ad-hoc TOML traversal for robustness.

3. Add `setup.py` fallback heuristics
   - File: `src/resolve/python.rs`
   - Add targeted parsing for common patterns (`find_packages(where="src")`, `package_dir={"": "src"}`) when no `pyproject.toml` signal exists.
   - Decision: skip ambiguous/dynamic cases rather than guessing.

4. Resolve absolute imports via discovered package map
   - File: `src/resolve/python.rs`
   - Map `import pkg.mod` and `from pkg.sub import name` to local files (`.py` / `__init__.py`) using package roots.
   - Decision: unresolved non-local packages remain external.

5. Add integration fixtures for layout variants
   - File: `tests/cli.rs`
   - Add fixtures for flat layout and `src/` layout with absolute imports.
   - Decision: verify through `kdb deps` output to capture end-user behavior.

### Files touched

```
┌──────────────────────┬────────────────────────────────────────────────────────────┐
│         File         │                           Action                           │
├──────────────────────┼────────────────────────────────────────────────────────────┤
│ src/resolve/mod.rs   │ Edit (add Python package cache model)                     │
│ src/resolve/python.rs│ Edit (manifest parsing + absolute import resolution)      │
│ Cargo.toml           │ Edit (add `pyproject-toml` dependency if needed)          │
│ Cargo.lock           │ Edit (lockfile update)                                     │
│ tests/cli.rs         │ Edit (Python package-layout deps integration tests)        │
└──────────────────────┴────────────────────────────────────────────────────────────┘
```

### Verification

- `cargo test --test cli deps_supports_python_`
- `cargo test --test cli deps_`
- `cargo test`
