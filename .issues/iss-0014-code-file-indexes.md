---
id: 14
title: Code File Index Headers
status: in_progress
priority: medium
labels:
  - feat
---

# 0014 :: Code File Index Headers

## Intent

Generate navigable index headers at the top of code files for jumping to methods, functions, classes, etc. Same pattern as markdown index tables with routable wikilinks.

## Example

```rust
// ## Index
//
// fn find_root()       L17
// fn resolve_path()    L45
// fn walk_ancestors()  L78
```

## Behavior

- `kdb fmt` should auto-generate/update index headers at the top of each file
- Should run on save (like a linter/formatter) via editor integration or file watcher
- Index is always in sync with the actual file contents — never manually maintained

## Scope (v1)

- Supported now: Rust (`.rs`), TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`), Python (`.py`), and Go (`.go`).
- Everything else is explicitly backlog for later issues.

## Implementation Plan

## Context

- Today, `kdb` has `init`, `check`, `outline`, and `lsp` commands, but no `fmt` command (`src/main.rs`, `src/cmd.rs`).
- We already have repo walking, root detection, and ignore pattern loading patterns to reuse (`src/root.rs`, `src/config.rs`, `src/index/mod.rs`).
- There is no code-file symbol extractor and no generated index-block rewrite path yet.
- Existing CLI and parser tests establish the style/pattern to follow (`tests/cli.rs`, `tests/index.rs`).

## Changes

1. Add `kdb fmt` command surface
   - File: `src/main.rs`, `src/cmd.rs`, `src/lib.rs`
   - Add a `Fmt` subcommand and a `cmd::fmt(...)` entrypoint.
   - Decision: keep command shape simple now (root auto-discovery + full workspace pass), then add flags later.

2. Add code index formatter core
   - File: `src/fmt/mod.rs` (new)
   - Discover supported code files, parse symbols, generate index header, and rewrite files in place.
   - Decision: use a human-readable compact list format (`keyword name() Lline`) instead of markdown table rows.

3. Add language + comment-style registry for v1 scope
   - File: `src/fmt/languages.rs` (new)
   - Map file extension -> language parser + comment style (`//` for Rust/TS/JS/Go, `#` for Python).
   - Decision: use tree-sitter grammars for Rust/TS/JS/Python/Go to avoid regex fragility and keep extraction consistent.

4. Implement deterministic generated-block protocol
   - File: `src/fmt/mod.rs` (new)
   - Normalize to one block shape with `{prefix} ## Index`, a blank comment line, then aligned symbol rows.
   - Decision: detect/update block by scanning preamble and matching the index header line; no sentinels.

5. Ensure accurate, stable line numbers
   - File: `src/fmt/mod.rs` (new)
   - Compute line numbers against final output form so inserted index lines do not drift symbol targets.
   - Decision: prioritize correctness/idempotency over micro-optimizing rewrite speed.

6. Add automated tests for formatter behavior
   - File: `tests/fmt.rs` (new)
   - Cover per-language extraction fixtures, stale-block replacement, insertion, and idempotent re-run.
   - Decision: include mixed-language workspace fixtures so one test run validates cross-language behavior.

7. Add CLI integration tests for `kdb fmt`
   - File: `tests/cli.rs` (edit)
   - Verify `kdb fmt` succeeds in a `.kdb` repo and rewrites supported files.
   - Decision: test one unsupported extension to assert safe skip behavior.

8. Update docs to match implemented behavior
   - File: `README.md` (edit)
   - Document that `kdb fmt` currently generates/updates code index headers for Rust, TS/JS, Python, and Go.
   - Decision: explicitly call out scope so users do not assume all languages are supported yet.

## Files touched

```
+--------------------------+------------------------------------------------+
| File                     | Action                                         |
+--------------------------+------------------------------------------------+
| src/main.rs              | Edit (add fmt subcommand)                      |
| src/cmd.rs               | Edit (add cmd::fmt entrypoint)                 |
| src/lib.rs               | Edit (export fmt module)                       |
| src/fmt/mod.rs           | Create (formatter workflow + rewrite logic)    |
| src/fmt/languages.rs     | Create (language registry + extractors)        |
| tests/fmt.rs             | Create (unit/integration formatter tests)      |
| tests/cli.rs             | Edit (fmt command integration tests)           |
| README.md                | Edit (document fmt behavior + language scope)  |
+--------------------------+------------------------------------------------+
```

## Verification

- Run `cargo test --test fmt` for formatter-specific coverage.
- Run `cargo test --test cli` to validate command wiring and end-to-end behavior.
- Run full suite with `cargo test`.
- Manual idempotency check in a fixture repo:
  1. run `kdb fmt`
  2. run `kdb fmt` again
  3. confirm second run produces no file changes.
- Manual accuracy check for each supported language: verify listed `Line` entries open expected declarations.

## Open Questions

- How deep should v2 symbol coverage go (e.g. constants, modules, macros, nested declarations) vs keeping the index focused on navigation-critical symbols?
- Should on-save integration live in LSP formatting hooks first, or via a standalone file watcher mode?
