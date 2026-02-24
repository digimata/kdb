---
id: 26
title: Workspace module resolution
status: in_progress
priority: high
labels:
  - feat
  - deps
  - refs
---

# ISS-0026 :: Workspace module resolution

## Intent

Build a per-file import map that resolves import statements to local file paths. This serves two consumers:

1. **`kdb deps`** — report resolved outbound dependencies
2. **`kdb refs -s`** (iss-0028) — use import maps to precisely resolve which symbol a bare identifier refers to, eliminating false positives in project-wide reference search

The import map is the foundation for precise code intelligence without a full language server.

## How it works

When indexing a file, parse its imports and build a map:

```
// src/router.rs
use crate::api::auth;        → import map: { "auth" → "src/api/auth.rs" }
use crate::root::find_root;  → import map: { "find_root" → "src/root.rs::find_root" }
```

When we encounter an identifier `find_root` in the AST, check the import map:
- Resolves → store as a confirmed reference to `src/root.rs::find_root`
- Doesn't resolve → local variable or built-in, not a reference to the target

This eliminates false positives where two files define identically-named symbols.

## Resolution sources (in priority order)

1. **Relative imports** (`./foo`, `../bar`) — resolve directly against filesystem
2. **tsconfig.json `paths`** — explicit path aliases (e.g. `@app/* → src/*`)
3. **pnpm-workspace.yaml** — `packages:` glob list → enumerate `package.json` names
4. **package.json `workspaces`** — array of glob patterns (npm/yarn)
5. **Cargo.toml** — `[workspace.members]` + `[dependencies]` with `path = "..."`
6. **go.work / go.mod** — workspace members + `replace` directives
7. **pyproject.toml / setup.py** — package discovery + editable installs

## Per-file import map

```rust
struct ResolvedImport {
    raw: String,                       // "@kernl-sdk/protocol"
    resolved_path: Option<PathBuf>,    // Some("packages/protocol/src/index.ts")
    kind: ImportKind,                  // Relative | Workspace | TsconfigPath | External
    names: Vec<String>,                // ["Agent", "AgentConfig"]
    line: usize,
}
```

The `names` field is critical for refs — it tells us which identifiers in this file came from which import.

## Resolver stack

| Language | Approach | Notes |
|---|---|---|
| TS/JS | `oxc_resolver` crate | Published, handles full Node resolution (package.json exports, tsconfig paths, pnpm/yarn/npm workspaces, scoped packages, subpath imports) |
| Rust | vendored `cargo_metadata` | Clone source into repo, no external dep. Stable, well-tested, thin enough to maintain in-tree |
| Go | handroll (~100 lines) | Parse `go.mod`/`go.work`, resolve module paths to local directories |
| Python | handroll (~150 lines) | Parse `pyproject.toml` + relative imports. Astral's `ty_module_resolver` (in ruff repo) as reference, but too coupled to their type checker to extract |

All resolvers implement a common trait in `src/resolve/mod.rs`. The index builder dispatches by file language.

**Phasing:** Phase 1 is TS/JS workspaces (most common for agents). Phase 2 adds Rust, Go, Python.

## Caching

The workspace package map (package-name → local-path) is computed once per `VaultIndex::build()`, shared across all file imports during indexing, and stored in VaultIndex so the LSP can reuse it. Invalidated when workspace config files change.

## Edge cases

- Scoped packages (`@kernl-sdk/protocol`) — match the full scoped name
- Subpath imports (`@kernl-sdk/protocol/agent`) — resolve through `exports` map or fall back to path guessing
- tsconfig paths (`@app/utils` → `src/utils`) — common in non-monorepo setups too
- Barrel files — report the entry point, not transitive deps
- Type-only imports (`import type { Foo }`) — still a reference for navigation purposes

## Current status

| Resolution source | Spec | Implemented | Notes |
|---|---|---|---|
| Relative imports (`./foo`, `../bar`) | yes | yes | Implemented across TS/JS, Rust, Go, Python resolvers |
| tsconfig `paths` | yes | yes | Implemented via `oxc_resolver` |
| `pnpm-workspace.yaml` | yes | yes | Package map discovery implemented |
| `package.json` `workspaces` | yes | yes | Package map discovery implemented |
| Cargo workspace members/path deps | yes | partial | Nested crate roots now work (nearest `Cargo.toml`), but cross-crate workspace dependency resolution is still missing |
| `go.mod` | yes | yes | Intra-module path resolution implemented |
| `go.work` | yes | partial | Not fully implemented/validated yet |
| `pyproject.toml` / `setup.py` | yes | no | Python resolver currently handles relative/local module-style imports only |

Rust crate-root bug noted earlier is fixed: resolver now discovers the nearest `Cargo.toml` and resolves from that crate's `src/` root.

## Remaining work

1. [ISS-0036](iss-0036-rust-workspace-module-resolution.md) — Rust workspace module resolution
2. [ISS-0037](iss-0037-go-workspace-module-resolution.md) — Go workspace module resolution (`go.work`)
3. [ISS-0038](iss-0038-python-package-discovery-resolution.md) — Python package discovery resolution

Parent issue done criteria:
- TS/JS path remains green
- ISS-0036, ISS-0037, and ISS-0038 are complete
- End-to-end `kdb deps` fixtures cover Rust multi-crate, Go multi-module (`go.work`), and Python package-layout resolution

## Changes

| File | Change |
|---|---|
| `src/resolve/mod.rs` (new) | Import resolution trait and shared types |
| `src/resolve/tsjs.rs` (new) | TS/JS workspace resolution (pnpm, npm, yarn, tsconfig paths) |
| `src/resolve/rust.rs` (new) | Rust crate/module resolution |
| `src/resolve/go.rs` (new) | Go module resolution |
| `src/resolve/python.rs` (new) | Python package resolution |
| `src/index/mod.rs` | Build import maps during VaultIndex construction |
