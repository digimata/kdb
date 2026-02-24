---
id: 18
title: kdb symbols Command
status: proposed
priority: high
labels:
  - feat
---

# 0018 :: kdb symbols Command

## Intent

Query the symbols in a single file from the CLI. Works on both markdown files (headings) and code files (functions, types, classes, etc.).

## Usage

```
kdb symbols <path>
kdb symbols <path> --json
kdb symbols <path> --public
```

## Output

Markdown:
```
$ kdb symbols docs/architecture.md
## Overview                     L1
## Data flow                    L15
### Indexing pipeline           L22
## Conventions                  L40
```

Code:
```
$ kdb symbols src/lsp/backend.rs
fn serve()                              L72
struct Backend                          L94
  fn Backend::new()                     L111
  fn Backend::ensure_index()            L148
  fn Backend::with_index()              L163
fn is_markdown_path()                   L451
```

## Flags

- `--json` — structured output for programmatic consumption
- `--public` — only public/exported symbols

## Implementation

- Markdown: build `VaultIndex`, look up the file, print headings with indentation based on level
- Code: reuse tree-sitter symbol extraction from `kdb fmt` (same `extract_symbols` call), format output

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Symbols` subcommand |
| `src/cmd.rs` | Add `cmd::symbols()` entrypoint |
| `src/cmd.rs` | Detect md vs code, dispatch to appropriate extraction |
