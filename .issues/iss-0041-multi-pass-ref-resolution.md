---
id: 41
title: "Multi-pass reference resolution pipeline"
status: done
priority: high
labels:
  - refactor
  - refs
---

# ISS-0041 :: Multi-pass reference resolution pipeline

Detailed design: [docs/specs/multi-pass-resolution.md](../docs/specs/multi-pass-resolution.md)

## Problem

The reference resolution pipeline in `code.rs` is single-pass: resolve
imports → build reexport lookup → scan usages → match. Individual fixes
(re-exports, qualified access, tsconfig paths) work in isolation but fail
when real code combines them.

Real-world example (mio):

```rust
// src/net/udp.rs
use crate::{event, sys, Interest, Registry, Token};  // grouped import
impl event::Source for UdpSocket {}                  // qualified access
```

This needs three steps to resolve:
1. Grouped import: `event` → `src/event/mod.rs`
2. Qualified access: `event::Source` → look up `Source` in `event/mod.rs`
3. Re-export following: `mod.rs` has `pub use self::source::Source` → `src/event/source.rs`

These don't compose because the pipeline only does one pass.

## Design

Inspired by rust-analyzer's `nameres` module (`crates/hir-def/src/nameres/`).
The key patterns: fixed-point resolution loop, per-module scope, and a clean
boundary between what name resolution can do vs what needs type inference.

### Core data structures

**`ModuleScope`** — per-file map of visible names. Replaces the current
`ImportedBindings` which is rebuilt per-file during `link_usage_refs`.

```rust
struct ModuleScope {
    /// name → target file(s) where the symbol is defined.
    names: HashMap<String, Vec<PathBuf>>,
    /// local alias → definition name.
    aliases: HashMap<String, String>,
}
```

Built from resolved imports and progressively enriched during the
fixed-point loop as re-exports and qualified access produce new bindings.

**`ImportDirective`** — an unresolved or partially-resolved import, kept
in a work queue until fully resolved.

```rust
enum ImportStatus {
    /// Path couldn't resolve yet — retry next iteration.
    Unresolved,
    /// Resolved to a file, but the target name isn't confirmed yet
    /// (e.g. target file might re-export it from elsewhere).
    Partial { file: PathBuf },
    /// Fully resolved to a definition site.
    Resolved { file: PathBuf, name: String },
}
```

The tri-state (borrowed from rust-analyzer's `PartialResolvedImport`)
allows partial progress: "I know `event` resolves to `src/event/mod.rs`,
but I don't know where `Source` lives yet."

### Pipeline phases

**Phase 1: Import collection (existing, unchanged)**

Per-language resolvers produce `Vec<ResolvedImport>` per file. This is
the raw material: specifier → file path + imported names. No changes
needed here (except 0039.8 per-package tsconfig resolvers for TS).

**Phase 2: Fixed-point resolution loop**

The core change. Iterate until stable:

```rust
fn resolution_loop(&mut self) -> ReachedFixedPoint {
    loop {
        let mut changed = false;
        changed |= self.resolve_reexports();
        changed |= self.expand_qualified_access();
        if !changed { break; }
    }
}
```

`ReachedFixedPoint` — a simple bool signal (from rust-analyzer). No depth
counter needed; convergence is guaranteed because each iteration either adds
a new binding or makes no progress. Cycle detection uses a visited set in
the re-export follower (already implemented).

**`resolve_reexports()`**: for each binding that points to a file where
the name doesn't exist as a symbol, check if that file re-exports it.
If so, update the binding to the re-export target. Returns `true` if
any binding was updated.

**`expand_qualified_access()`**: for each binding that points to a module
file (not a specific symbol), scan usages for qualified patterns
(`module::Symbol`, `module.symbol`). Create new bindings from the accessed
symbol name to the module's file. These new bindings may need re-export
following on the next iteration. Returns `true` if any new binding was added.

**Phase 3: Usage matching (existing, mostly unchanged)**

`UsageScanner` matches identifiers against the fully-resolved bindings.
The scanner itself doesn't change — it just gets better bindings as input.

**Phase 4: Go same-package refs (existing, unchanged)**

Orthogonal to the resolution loop. Runs after phase 3.

### Glob import propagation (0039.7)

Borrow rust-analyzer's reverse index pattern for wildcard imports
(`use crate::*`, `from x import *`):

```rust
/// Modules that glob-import from a given module.
glob_importers: HashMap<PathBuf, Vec<PathBuf>>
```

When a name is added to module M's scope during the loop, propagate it
to all modules that glob-import from M. The `changed == false` convergence
check handles cycles (e.g. `mod a { pub use b::*; } mod b { pub use a::*; }`).

This runs inside the fixed-point loop alongside `resolve_reexports()` and
`expand_qualified_access()`:

```rust
fn resolution_loop(&mut self) -> ReachedFixedPoint {
    loop {
        let mut changed = false;
        changed |= self.resolve_reexports();
        changed |= self.expand_qualified_access();
        changed |= self.propagate_glob_imports();
        if !changed { break; }
    }
}
```

`propagate_glob_imports()`: for each glob import (`use foo::*` /
`from foo import *`), look up all exported names in the target module's
scope and add them to the importing module's scope. Uses `__all__` when
present (Python). Returns `true` if any new names were added.

Per-language glob patterns:
- **Rust**: `use crate::foo::*` — all `pub` items from `foo`
- **Python**: `from foo import *` — names in `__all__`, or all non-`_`
  prefixed names if `__all__` is absent
- **Go**: dot imports (`import . "pkg"`) — already handled separately
- **TS/JS**: `export * from './foo'` in barrel files — already handled
  as re-exports, not user-facing glob imports

### Segment-by-segment path resolution

For qualified paths (`a::b::c` in Rust, `a.b.c` in TS/Python/Go), resolve
segment by segment through modules:

1. Resolve `a` → module file
2. Look up `b` in that module's scope → if it's a module, continue
3. Look up `c` in `b`'s scope → if it's a non-module symbol, stop

The stopping point is the **type-checker boundary** (rust-analyzer calls
this `segment_index`). Everything before it is name resolution; everything
after requires type inference. We stop and emit what we have.

### What this replaces in the current code

| Current | New |
|---|---|
| `ImportedBindings::from_imports()` per file in `link_usage_refs` | `ModuleScope` built once, enriched in loop |
| `build_reexport_lookup()` + `follow_reexport_target()` | `resolve_reexports()` inside the loop |
| Qualified access in `UsageScanner` only | `expand_qualified_access()` creates bindings *before* usage scanning |
| Single pass through all files | Fixed-point loop until stable, then usage scan |

## Acceptance criteria

Test against real projects:

| Project | Lang | Symbol | Current | Target |
|---|---|---|---|---|
| mio | Rust | `Source` | 2 | 12+ (all `impl event::Source`) |
| tokio | Rust | `Handle` | 14 | 20+ |
| poetry | Python | `Locker` | 60 | 60+ (already good) |
| n8n | TS | `Server` | 1 | 2+ (tsconfig path alias) |

Eval suite stays at 38/38.

## Supersedes / subsumes

- **0039.5** (re-export following) — done, but becomes part of the loop
- **0039.6** (namespace/qualified access) — done, but becomes part of the loop
- **0039.7** (wildcards) — `propagate_glob_imports()` in the loop
- **0039.8** (tsconfig paths) — separate fix (per-package resolvers), but
  benefits from the loop for compound cases

## Scope

Primary change site: `src/index/code.rs`

The per-language resolvers (`src/resolve/*.rs`) don't change — phase 1
stays as-is. The `UsageScanner` mostly stays as-is — it just gets better
input bindings.

New code:
- `ModuleScope` struct + builder
- `resolution_loop()` with `resolve_reexports()` and `expand_qualified_access()`
- `ReachedFixedPoint` signal type

Removed code:
- `ImportedBindings` (replaced by `ModuleScope`)
- Inline re-export following in `insert_usage_row` (moved into the loop)

## Design references

- **rust-analyzer** `crates/hir-def/src/nameres/collector.rs` — fixed-point
  resolution loop, glob propagation, `ReachedFixedPoint` signal
- **rust-analyzer** `crates/hir-def/src/item_scope.rs` — per-module name
  storage, `fully_resolve_import()` chain following
- **ruff/ty** `crates/ty_python_semantic/src/semantic_index/re_exports.rs` —
  `exported_names()` pattern for "what does this module export?"
- **turborepo** `crates/turbo-trace/src/tracer.rs` — per-package resolver
  construction with explicit tsconfig paths
