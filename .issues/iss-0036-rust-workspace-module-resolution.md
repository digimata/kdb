---
id: 36
title: Rust workspace module resolution
status: proposed
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
