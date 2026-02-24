---
id: 16
title: Codemap
status: proposed
priority: high
labels:
  - feat
---

# 0016 :: Codemap

## Intent

Provide a unified, agent-readable map of the codebase that combines an authored architecture overview with auto-generated per-file summaries. The goal is optimal agent orientation — an agent reads the codemap first and immediately knows what's here, how it fits together, and where to go for any given task.

## Artifacts

### `CODEMAP.md` (authored, repo root)

The high-level stuff that can't be derived — architecture, data flow, design decisions, conventions. Written by whoever's doing the work (agent or human). Enforced by CI (headless Claude checking conformity on every push).

### `kdb codemap` (read-only command)

Assembles a unified map by combining `CODEMAP.md` with per-file summaries derived from module doc comments and `kdb fmt` index headers. Pure derivation, no state.

## Usage

```
kdb codemap [path]       # print codemap to stdout
kdb codemap --check      # exit 1 if codemap is stale or incomplete
```

- `path` defaults to project root. If a subdirectory is given, only that subtree is mapped.
- Output goes to stdout.

## Output format

Two sections: the authored preamble from `CODEMAP.md`, then the generated per-file map.

```
<contents of CODEMAP.md>

---

## File map

### src/cmd.rs
CLI subcommand dispatch.

  fn init()                    L28
  fn check()                   L73
  fn outline()                 L91
  fn fmt()                     L147
  fn lsp()                     L167

### src/fmt/languages.rs
Language-specific symbol extraction.

  enum CodeLanguage              L39
  enum SymbolKind                L50
    fn SymbolKind::as_str()      L63
  struct Symbol                  L79
  fn language_for_path()         L89
  fn extract_symbols()           L115
  ...
```

## How it works

1. Read `CODEMAP.md` from project root — emit as preamble
2. Walk the target path, discover supported code files (same walker as `kdb fmt`)
3. For each file:
   - Extract the module-level doc comment (first `//!` block in Rust, `#` docstring in Python, etc.) — use first line as summary
   - Read the existing `## Index` block if present — emit symbol lines
   - If no index block, run symbol extraction on the fly
4. Emit files grouped by directory, sorted alphabetically

## `--check` mode

Validates that the codemap is complete and current. Exits non-zero if:

- `CODEMAP.md` doesn't exist at project root
- A supported code file has no module-level doc comment
- A supported code file has a stale or missing `## Index` block (same check as `kdb fmt --check`)

This is what headless Claude runs in CI to enforce codemap conformity.

## Relationship to `kdb fmt`

- `kdb fmt` **writes** the per-file index blocks into source files
- `kdb codemap` **reads** those blocks (and doc comments) to produce the unified map
- `kdb codemap --check` is a superset of `kdb fmt --check` — it also validates doc comments and `CODEMAP.md` existence

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Codemap` subcommand |
| `src/cmd.rs` | Add `cmd::codemap()` entrypoint |
| `src/codemap.rs` (new) | Codemap assembly — read CODEMAP.md, walk files, extract summaries + indexes |
| `CODEMAP.md` (new) | Authored architecture doc |
| `README.md` | Add `kdb codemap` to commands list |
| `tests/cli.rs` | Integration tests for codemap command |

## Open questions

- Should `kdb codemap` also map markdown files (using heading outlines from `kdb outline`), or only code files?
- Should the file map include files that `kdb fmt` doesn't support (e.g. `.toml`, `.json`) with just the summary line and no symbol index?
- What's the right granularity for `--check` failures — warn vs error for missing doc comments?
