---
id: 44
title: "symbols: support multiple path arguments"
status: done
priority: medium
labels:
  - enhancement
  - symbols
---

# ISS-0044 :: symbols: support multiple path arguments

## Problem

`kdb symbols` accepts a single path. It should accept multiple paths so you can get a concatenated view of symbols across several files and/or directories in one invocation.

## Desired behavior

```
$ kdb symbols src/resolve/mod.rs src/resolve/rust.rs src/symbols/
── src/resolve/mod.rs
pub struct ResolvedImport                       L132
pub(crate) struct ReexportBinding               L145
...

── src/resolve/rust.rs
pub(crate) struct RustResolver                  L58
  pub(super) fn new()                           L65
...

── src/symbols/mod.rs
pub enum SymbolKind                             L41
pub struct Symbol                               L63
...

── src/symbols/display.rs
pub struct SymbolRow                            L31
...
```

Each argument is expanded independently — files are printed directly, directories are recursed (per iss-0042). Results are concatenated in argument order.

## Design considerations

- Builds on top of iss-0042 (directory support) — this adds the multi-arg layer
- Dedup: if a file appears in multiple arguments (e.g. explicit file + containing directory), show it once
- CLI: change `<PATH>` to `<PATH>...` (variadic positional)
- `--json` output: array of symbol objects, each with file path — same as single-path, just concatenated
- `-s` selector should filter across all paths
