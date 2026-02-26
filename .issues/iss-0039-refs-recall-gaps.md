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

6 tests excluded as out of scope (see below). In-scope total: **38**.

| Language | Pass | Fail | In-scope | Recall |
|---|---|---|---|---|
| Rust | 11 | 0 | 11 | 100% |
| TS/JS | 10 | 0 | 10 | 100% |
| Python | 10 | 0 | 10 | 100% |
| Go | 7 | 0 | 7 | 100% |
| **Total** | **38** | **0** | **38** | **100%** |

## Sub-issues

### Done (archived to GitHub #37–#45)

0039.1 (Python symbol binding), 0039.2 (Go namespace access), 0039.3 (usage scanner gaps), 0039.4 (alias tracking), 0039.5 (re-export following), 0039.6 (namespace access), 0039.7 (wildcard imports), 0039.8 (tsconfig path aliases), 0039.9 (Go same-package refs), 0039.12 (Rust scoped imports), 0039.14 (TS member access on named import).

### Open

| Issue | Gap | Affects | Priority |
|---|---|---|---|
| [0039.10 — TSX/JSX real-world](iss-0039/iss-0039.10-tsx-jsx-real-world.md) | JSX tag/expr/call identifiers not found in real TSX | real-world TSX | medium |
| [0039.11 — Re-export as reference](iss-0039/iss-0039.11-reexport-as-reference.md) | `export { foo }` re-export not counted as usage | real-world TS/Rust | medium |
| [0039.13 — Rust cfg-macro symbols](iss-0039/iss-0039.13-rust-cfg-macro-symbols.md) | Symbols inside `cfg_if!` / token-tree not extracted | real-world Rust | medium |

## Out of scope

These require type inference, macro expansion, or runtime analysis — none of which `refs -s` attempts. No other kdb command needs these capabilities either.

| Category | Why out of scope | Tests |
|---|---|---|
| Type-based method dispatch | `conn.execute()` requires knowing the type of `conn` | R6, R7, G4, G5 |
| Macro expansion | `macro_rules!` generates code invisible to tree-sitter | R8 |
| Dynamic imports | `await import('./foo')` is a runtime construct | T7 |
