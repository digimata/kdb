---
id: 37
title: Go workspace module resolution (go.work)
status: proposed
priority: high
labels:
  - feat
  - deps
  - refs
  - go
---

# ISS-0037 :: Go workspace module resolution (`go.work`)

## Intent

Complete Go workspace-aware import resolution by supporting multi-module workspaces declared in `go.work`.

Current state from iss-0026:
- `go.mod` module-prefix resolution works for a single module
- `go.work` member/replacement semantics are not fully implemented

## Scope

1. Parse `go.work`
   - `use` entries (workspace modules)
   - `replace` entries to local paths
2. Build workspace module map
   - module path -> local directory (with `replace` precedence)
3. Resolve imports across workspace modules
   - `example.com/modb/pkg` from files in `example.com/moda`

## Out of scope

- Network/module proxy resolution
- Versioned external module fetching

## Done when

- `kdb deps` resolves imports across modules in a `go.work` fixture.
- `replace` to local path is honored when computing resolved file targets.
- Imports outside workspace/local replacements remain external/unresolved.
- Tests cover:
  - single-module behavior (no regression)
  - multi-module `go.work` `use`
  - local `replace` path override

## Expected changes

| File | Change |
|---|---|
| `src/resolve/go.rs` | Add `go.work` parsing + cross-module resolution |
| `src/resolve/mod.rs` | Add/extend workspace cache structures for Go modules |
| `tests/cli.rs` | Add Go multi-module workspace deps tests |
