---
id: 37
title: Go workspace module resolution (go.work)
status: done
priority: medium
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

## Implementation plan

### Context

- Current Go behavior resolves imports within a single `go.mod` module prefix.
- `go.work` is currently medium priority and can land as a focused follow-up.
- Rust crate ecosystem support for full `go.work` semantics is limited; a targeted parser is the pragmatic path.

### Changes

1. Add Go workspace cache model
   - File: `src/resolve/mod.rs`
   - Add optional cached map of module path -> local directory built at index time.
   - Decision: keep cache language-specific and independent from TS/Rust/Python structures.

2. Implement `go.work` parsing (targeted subset)
   - File: `src/resolve/go.rs`
   - Parse `use` (single and block forms) and `replace` directives with local path replacements.
   - Decision: support the common local-workspace forms first; ignore remote version replacements.

3. Merge `go.mod` and `go.work` data into module resolution
   - File: `src/resolve/go.rs`
   - Resolve imports by longest module-prefix match from workspace map, then locate package files under matched module roots.
   - Decision: preserve existing single-module behavior when `go.work` is absent.

4. Add end-to-end fixtures for workspace resolution
   - File: `tests/cli.rs`
   - Add multi-module fixtures that exercise `go.work use` and `replace` to local paths.
   - Decision: continue validating via `kdb deps` output to ensure stable CLI behavior.

### Files touched

```
┌────────────────────┬──────────────────────────────────────────────────────────────┐
│        File        │                            Action                            │
├────────────────────┼──────────────────────────────────────────────────────────────┤
│ src/resolve/mod.rs │ Edit (add Go workspace cache model)                         │
│ src/resolve/go.rs  │ Edit (`go.work` parsing + cross-module resolution)          │
│ tests/cli.rs       │ Edit (Go workspace deps integration tests)                  │
└────────────────────┴──────────────────────────────────────────────────────────────┘
```

### Verification

- `cargo test --test cli deps_supports_go_`
- `cargo test --test cli deps_`
- `cargo test`
