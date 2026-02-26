---
id: 39
title: "refs -s recall gaps (tracking)"
status: in_progress
priority: high
labels:
  - tracking
  - refs
parent: 28
---

# ISS-0039 :: `refs -s` recall gaps (tracking)

Tracking issue for recall gaps discovered by the correctness eval suite (`tests/refs_eval.rs`).

`refs -s` does **name resolution** — tracing imported symbol names from definition to usage across files. It does not do type inference, macro expansion, or runtime analysis. The scorecard below reflects only the categories that are in scope.

## Scorecard

6 tests excluded as out of scope (see below). In-scope total: **31**.

| Language | Pass | Fail | In-scope | Recall |
|---|---|---|---|---|
| Rust | 5 | 2 | 7 | 71% |
| TS/JS | 7 | 3 | 10 | 70% |
| Python | 5 | 4 | 9 | 56% |
| Go | 4 | 1 | 5 | 80% |
| **Total** | **22** | **9** | **31** | **71%** |

Target after all sub-issues resolved: **31/31 (100%)**.

## Sub-issues

| Issue | Gap | Affects | Priority |
|---|---|---|---|
| [0039.1 — Python symbol binding](iss-0039.1-python-symbol-binding.md) | `from X import name` resolves name as submodule, not symbol | P1, P6, P7, P8, P9 | high |
| [0039.2 — Go namespace access](iss-0039.2-go-namespace-access.md) | `pkg.Foo` — import names contain pkg alias, not symbol names | G1, G6, G7 | high |
| [0039.3 — Usage scanner gaps](iss-0039.3-usage-scanner-gaps.md) | Parameter type filter (R9) + JSX identifier kind (T11) | R9, T11 | medium |
| [0039.4 — Alias tracking](iss-0039.4-alias-tracking.md) | Alias name in bindings, definition name in symbol_lookup | R3, T3, P3, G2 | medium |
| [0039.5 — Re-export following](iss-0039.5-reexport-following.md) | `pub use`, barrel files, `__init__.py` not followed | R4, T5, T6, P5 | high |
| [0039.6 — Namespace access](iss-0039.6-namespace-access.md) | `X.name` qualified access not decomposed | T4, P2, G3 | medium |
| [0039.7 — Wildcard imports](iss-0039.7-wildcard-imports.md) | `use X::*` / `from X import *` not expanded | R5, P4 | low |

## Out of scope

These require type inference, macro expansion, or runtime analysis — none of which `refs -s` attempts. No other kdb command needs these capabilities either.

| Category | Why out of scope | Tests |
|---|---|---|
| Type-based method dispatch | `conn.execute()` requires knowing the type of `conn` | R6, R7, G4, G5 |
| Macro expansion | `macro_rules!` generates code invisible to tree-sitter | R8 |
| Dynamic imports | `await import('./foo')` is a runtime construct | T7 |
