---
id: 36
title: Rust workspace module resolution
status: done
priority: high
labels:
  - feat
  - deps
  - refs
  - rust
---

# ISS-0036 :: Rust workspace module resolution

## Intent

Finish Rust import resolution for workspaces by resolving cross-crate imports from local Cargo workspace metadata.

Current state from iss-0026:
- Intra-crate resolution works (`crate::`, `self::`, `super::`, local `mod`)
- Nested crate roots work (nearest `Cargo.toml`)
- Cross-crate workspace imports are not resolved yet

## Scope

1. Parse Cargo workspace topology
   - `[workspace.members]`
   - member crate names from each `Cargo.toml`
2. Parse local path dependencies
   - `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`
   - support `package = "..."` rename aliases
3. Resolve Rust imports that start with a dependency crate name to local files
   - `use sibling_crate::foo::bar`
   - map crate name -> crate `src/` root -> module file

## Out of scope

- Full Cargo feature/target cfg evaluation
- Registry/git dependencies (remain external)

## Done when

- `kdb deps` resolves cross-crate Rust imports in a multi-crate workspace fixture.
- Renamed path deps (`foo = { package = "bar", path = "..." }`) resolve via alias.
- External crates still classify as external/unresolved.
- Tests cover:
  - same-crate imports (no regression)
  - sibling workspace crate imports
  - renamed local dependency imports

## Expected changes

| File | Change |
|---|---|
| `src/resolve/rust.rs` | Add cross-crate workspace import resolution |
| `src/resolve/mod.rs` | Add/extend workspace cache structures for Rust crate maps |
| `tests/cli.rs` | Add `kdb deps` multi-crate Rust integration tests |

## Implementation plan

### Context

- The Rust resolver now correctly discovers the nearest crate root (`Cargo.toml`) and resolves intra-crate imports.
- Cross-crate imports in workspaces still resolve as external because we do not have a workspace crate map.
- The most robust source of Cargo workspace/dependency truth is `cargo metadata` (via `cargo_metadata`).

### Changes

1. Add Rust workspace cache model
   - File: `src/resolve/mod.rs`
   - Add a cache structure for Rust crates and dependency alias maps, built once per `VaultIndex::build()`.
   - Decision: keep cache optional and best-effort so indexing still works when metadata cannot be read.

2. Build Cargo workspace graph from `cargo metadata`
   - File: `src/resolve/rust.rs`
   - Use `cargo_metadata::MetadataCommand` (`no_deps`) to read workspace members, crate names, and local path dependencies.
   - Decision: rely on Cargo's own metadata model for correctness over hand-parsing many `Cargo.toml` edge cases.

3. Resolve cross-crate `use` prefixes through workspace dependency aliases
   - File: `src/resolve/rust.rs`
   - For `use foo::bar`, map `foo` through dependency aliases (`package = "..."`) to a local crate, then resolve module files under that crate's `src/` root.
   - Decision: only resolve crates proven local via workspace/path deps; keep registry/git deps as external.

4. Add regression and fixture coverage
   - File: `tests/cli.rs`
   - Add multi-crate workspace fixtures for direct and renamed path dependencies.
   - Decision: verify via `kdb deps` CLI output to lock user-visible behavior.

### Files touched

```
┌────────────────────┬──────────────────────────────────────────────────────────────┐
│        File        │                            Action                            │
├────────────────────┼──────────────────────────────────────────────────────────────┤
│ src/resolve/mod.rs │ Edit (add Rust workspace cache model)                       │
│ src/resolve/rust.rs│ Edit (cargo metadata ingest + cross-crate resolution)       │
│ Cargo.toml         │ Edit (add `cargo_metadata` dependency if not vendored)      │
│ Cargo.lock         │ Edit (lockfile update)                                       │
│ tests/cli.rs       │ Edit (Rust multi-crate deps integration tests)              │
└────────────────────┴──────────────────────────────────────────────────────────────┘
```

### Verification

- `cargo test --test cli deps_supports_rust_`
- `cargo test --test cli deps_`
- `cargo test`
