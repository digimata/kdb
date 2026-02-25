---
title: "ISS-0028 v2: refs context lines (`-c`)"
date: 2026-02-25
status: draft
affects: "refs CLI surface, symbol-ref text rendering, CLI tests"
---

## Context

`refs -s` currently prints one line per symbol reference (`file:line:col  snippet`) via
`src/index/refs.rs::print_symbol_refs_text`. There is no way to inspect surrounding source
without opening files separately.

The v2 request is to add `-c/--context <N>` (grep-style context windows) so text output can show
N lines before/after each reference line, with the match line highlighted.

Current structure after refactor:
- `src/main.rs` defines `Refs { target, symbol, json, count }`.
- `src/cmd.rs::refs(...)` branches markdown refs vs symbol refs.
- Symbol refs live in `CodeIndex.symbols.refs` (`SymbolIndex`).
- `src/index/refs.rs` owns symbol-ref querying and text rendering.

## Changes

1) Extend `refs` CLI with typed context option
- **Files:** `src/main.rs`, `src/cmd.rs`
- Add `-c/--context <N>` to `Refs`.
- Thread context into `cmd::refs(...)` and gate behavior:
  - `-s` + text output: render context.
  - `--json` or `--count`: keep existing behavior (no context rendering).
  - without `-s`: return a clear error (`--context currently supported only with --symbol`).
- Key decision: constrain v2 scope to symbol refs first, matching issue intent and minimizing
  behavioral risk for markdown refs.

2) Add type-driven context rendering model for symbol refs
- **File:** `src/index/refs.rs` (or `src/index/refs_render.rs` if extraction improves size)
- Introduce small domain types instead of ad-hoc formatting:
  - `SymbolRefRenderOptions { context_lines: usize }`
  - `ContextWindow { start_line, end_line, match_line }`
  - `SourceContext { lines: Vec<String> }`
  - `SymbolRefTextRenderer` with `impl` methods for grouping/rendering.
- Add a shared behavior trait for line retrieval if needed by tests and future renderers:
  - `trait LineSource { fn lines_for(&mut self, rel: &Path) -> Result<&[String]>; }`
  - `FsLineSource` production implementation (cached file reads).
- Key decision: use structs + impls (CC-3.3) so context windowing and printing rules are explicit,
  testable, and easy to extend.

3) Implement context-aware text output semantics
- **File:** `src/index/refs.rs`
- Keep current one-line output when `context_lines == 0`.
- For `context_lines > 0`:
  - print a per-reference header (`file:line:column`),
  - print `N` lines before and after,
  - mark the match line with a distinct prefix (e.g. `>`),
  - include line numbers for all displayed lines,
  - separate reference blocks with `--`.
- v2 default: do **not** merge overlapping windows. Keep one block per reference for predictable
  output; optimize/merge in a later follow-up if needed.

4) Add focused test coverage for `-c`
- **File:** `tests/cli.rs`
- Add integration tests:
- `refs -s <name> -c 1` prints surrounding lines and highlights match lines.
- `refs -s <name> -c 0` matches current one-line output behavior.
- `refs <markdown-target> -c 1` errors with clear scope message.
- `refs -s <name> --json -c 2` preserves existing JSON shape (no context payload).
- `refs -s <name> --count -c 2` still prints numeric count only.

5) Keep issue docs aligned with shipped behavior
- **File:** `.issues/iss-0028-code-symbol-refs.md`
- Update the v2 context section wording to reflect actual semantics from implementation:
  - symbol refs only for now,
  - text-mode only,
  - explicit behavior for `--json`/`--count`.

## Files touched

```text
┌─────────────────────────────────────────────┬────────────────────────────────────────────────────────────┐
│                    File                     │                           Action                           │
├─────────────────────────────────────────────┼────────────────────────────────────────────────────────────┤
│ .issues/plans/iss-0028-v2-context-plan.md  │ Create implementation plan for context-line feature        │
│ src/main.rs                                │ Edit add `-c/--context` to `refs` subcommand              │
│ src/cmd.rs                                 │ Edit thread context option + mode validation               │
│ src/index/refs.rs                          │ Edit add typed symbol-ref context renderer                 │
│ tests/cli.rs                               │ Edit add `refs -s -c` integration coverage                 │
│ .issues/iss-0028-code-symbol-refs.md       │ Edit clarify finalized v2 context semantics                │
└─────────────────────────────────────────────┴────────────────────────────────────────────────────────────┘
```

## Verification

1. `cargo fmt`
2. `cargo test --test cli refs_symbol`
3. `cargo test --test cli refs_` (broader refs regressions)
4. `cargo test`
5. Manual sanity checks:
   - `kdb refs src/project/root.rs -s find_root -c 1`
   - `kdb refs src/project/root.rs -s find_root --json -c 1`
   - `kdb refs src/project/root.rs -s find_root --count -c 1`
   - `kdb refs docs/page.md -c 1` (verify clear error path)
