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

## Scorecard (baseline)

| Language | Pass | Fail | Ignored | Total | Recall |
|---|---|---|---|---|---|
| Rust | 3 | 1 | 6 | 10 | 30% |
| TS/JS | 5 | 1 | 5 | 11 | 45% |
| Python | 0 | 5 | 4 | 9 | 0% |
| Go | 0 | 3 | 4 | 7 | 0% |
| **Total** | **8** | **10** | **19** | **37** | **22%** |

## Sub-issues

| Issue | Gap | Affects | Priority |
|---|---|---|---|
| [0039.1 — Python symbol binding](iss-0039.1-python-symbol-binding.md) | `from X import name` resolves name as submodule, not symbol | P1, P6, P7, P8, P9 | high |
| [0039.2 — Go namespace access](iss-0039.2-go-namespace-access.md) | `pkg.Foo` — import names contain pkg alias, not symbol names | G1, G6, G7 | high |
| [0039.3 — Usage scanner gaps](iss-0039.3-usage-scanner-gaps.md) | Parameter type filter (R9) + JSX identifier kind (T11) | R9, T11 | medium |

## Known gaps (ignored tests, not yet tracked)

These are architectural limitations that require deeper work. Not yet broken out into sub-issues.

| Category | Gap | Tests |
|---|---|---|
| Aliased imports | Alias name stored in `ResolvedImport.names`, `symbol_lookup` has definition name | R3, T3, P3, G2 |
| Re-exports | `pub use`, barrel files, `__all__` not followed transitively | R4, T5, T6, P5 |
| Wildcard imports | `use X::*` / `from X import *` not expanded | R5, P4 |
| Namespace access | `import X; X.foo()` (Python), `import . "pkg"` (Go dot import) | P2, G3 |
| No type tracking | Method calls on imported types, trait dispatch, interface methods, embedded structs | R6, R7, G4, G5 |
| No macro expansion | Macro-generated symbol usages | R8 |
| Dynamic imports | `await import('./foo')` | T7 |
