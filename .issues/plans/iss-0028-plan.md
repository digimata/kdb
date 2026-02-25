---
title: "Code symbol references (`refs -s`)"
date: 2026-02-25
status: draft
affects: "refs command, code index, symbol/query pipeline"
---

## Context

`kdb refs` currently resolves markdown inbound links only (`<file>` and `<file>#<heading>`).
Issue `iss-0028` adds code symbol references via `kdb refs <file> -s <symbol>`, with the same
output modes (`text`, `--json`, `--count`).

The repo already has the core primitives needed for import-resolved matching:
- `CodeIndex.code_imports` with resolved local import paths + imported names.
- Multi-language declaration extraction via `symbols::extract_symbols`.
- Existing refs model in `index/refs.rs` (`collect_inbound`, sorted output, text/json/count behavior).

What is missing is the bridge between declaration symbols and usage sites:
identifier usage extraction, import-resolved matching, and a query surface for symbol refs.

Decision for v1: implement symbol-ref indexing lazily (only for `refs -s`) so existing commands
(`deps`, markdown `refs`) keep current performance characteristics.

## Changes

### 1. Add `refs -s` CLI surface and command branching

- **Files:** `src/main.rs`, `src/cmd.rs`
- Extend `Refs` subcommand with `-s/--symbol <name>`.
- Update `cmd::refs(...)` to branch:
  - no `--symbol`: keep existing markdown refs flow unchanged.
  - with `--symbol`: resolve `<file>` as a supported code file and run symbol-ref query path.
- Keep `--json` and `--count` behavior identical across both modes.

Trade-off:
- Single command with mode switch avoids introducing a second command and keeps UX aligned with issue intent.

### 2. Extend code index data model for symbol references

- **File:** `src/index/mod.rs`
- Add `CodeIndex` fields:
  - `code_symbols: BTreeMap<PathBuf, Vec<Symbol>>`
  - `symbol_refs: HashMap<SymbolKey, Vec<SymbolRef>>`
- Add new types:
  - `SymbolKey { file, name, parent, kind, line }`
  - `SymbolRef { source_file, line, column, snippet, is_definition }`
- Add a lazy builder path (e.g. `CodeIndex::build_with_symbol_refs(...)`) used only by `refs -s`.

Trade-off:
- Lazy build does extra work only when requested; avoids regressing `deps` and other commands.

### 3. Implement identifier-usage extraction + import-resolved matching

- **File (new):** `src/index/code_refs.rs`
- Build pipeline for `refs -s` mode:
  1. Read sources for indexed code files.
  2. Extract declarations (`symbols::extract_symbols`) per file.
  3. Extract identifier usage nodes per file (tree-sitter walk, declarations/import nodes excluded).
  4. Match usages to imported names from `ResolvedImport.names`.
  5. Resolve matched names to target file declarations and populate `symbol_refs`.
  6. Add declaration-site refs (`is_definition = true`) for each definition symbol.

Scope limits (explicit for v1):
- Direct imports only.
- No wildcard-import expansion, no re-export chains, no type-based dispatch.
- No cross-file re-export following (e.g. `export { Foo } from './foo'` in TS/JS barrel files). This is extremely common in TS/JS codebases and users will hit it quickly — high priority for v2.

Trade-off:
- Keeps implementation deterministic and fast while covering the high-value direct-import case.

### 4. Add symbol-ref query and rendering API in refs module

- **File:** `src/index/refs.rs`
- Keep markdown `parse_target`/`collect_inbound` as-is.
- Add symbol query function (e.g. `collect_symbol_refs(...)`) that:
  - validates target file + symbol exists,
  - returns sorted refs for matching `SymbolKey` entries,
  - supports text/json/count output path parity.
- Add text renderer for `SymbolRef` rows with `snippet` output.

Trade-off:
- Reusing `index::refs` as shared query surface keeps CLI printing logic consistent.

### 5. Add tests for symbol refs and regression coverage

- **Files:** `tests/cli.rs`, `tests/index.rs`
- Integration tests:
  - `kdb refs <code-file> -s <name>` returns definition + inbound usages.
  - `--count` returns the same row count as text/json mode.
  - `--json` includes `line`, `column`, `snippet`, `is_definition`.
  - error paths: unknown symbol, unsupported target file type.
- Unit tests:
  - import-resolved matching does not link unrelated same-name symbols.
  - declaration row inclusion behavior is stable.
- Regression test:
  - existing markdown `refs` behavior remains unchanged.

## Files touched

```text
┌───────────────────────────────┬──────────────────────────────────────────────────────────────┐
│             File              │                            Action                            │
├───────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ .issues/plans/iss-0028-plan.md │ Create implementation plan                                  │
│ src/main.rs                   │ Edit add `--symbol` flag to `refs` subcommand               │
│ src/cmd.rs                    │ Edit branch markdown refs vs symbol refs                     │
│ src/index/mod.rs              │ Edit add symbol-ref index types and lazy build entrypoint    │
│ src/index/code_refs.rs        │ Create identifier usage extraction + import-resolved matcher │
│ src/index/refs.rs             │ Edit add symbol-ref query + text output                      │
│ tests/cli.rs                  │ Edit add `kdb refs -s` integration coverage                  │
│ tests/index.rs                │ Edit add symbol-ref unit coverage                            │
└───────────────────────────────┴──────────────────────────────────────────────────────────────┘
```

## Verification

1. `cargo fmt`
2. `cargo test --test index`
3. `cargo test --test cli refs_`
4. `cargo test`
5. Manual sanity run in repo fixture:
   - `kdb refs <code-file> -s <symbol>`
   - `kdb refs <code-file> -s <symbol> --count`
   - `kdb refs <code-file> -s <symbol> --json`
