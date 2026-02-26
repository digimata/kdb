---
id: 42
title: "symbols: support directory targets with recursive aggregation"
status: proposed
priority: medium
labels:
  - enhancement
  - symbols
---

# ISS-0042 :: symbols: support directory targets with recursive aggregation

## Problem

`kdb symbols <path>` only accepts a single file. When `<path>` is a directory, it should aggregate symbols from all supported files under that directory — useful for getting an overview of a module or package at a glance.

## Desired behavior

```
$ kdb symbols src/symbols/
── src/symbols/mod.rs
pub mod display                    L9
mod extract                        L10
pub(crate) mod query               L30
mod tree                           L31
pub enum SymbolKind                L41
pub struct Symbol                  L63
pub(super) struct Extractor        L88
  pub(super) fn new()              L95
  pub(super) fn push()             L112
  pub(super) fn finish()           L151
pub fn extract_symbols()           L157

── src/symbols/display.rs
pub struct SymbolRow               L31
  fn from()                        L59
pub struct SymbolBodyRow           L78
pub fn kind_label()                L102
pub fn is_callable_kind()          L123
pub fn format_symbol_display()     L135
pub fn extract_symbol_body()       L163
pub fn extract_body_with_docs()    L180
pub fn code_symbol_body_row()      L242
pub fn markdown_symbol_body_row()  L258
pub fn print_text()                L278
pub fn print_bodies_text()         L291

── src/symbols/query.rs
pub fn collect_rows()              L29
pub fn collect_body_rows()         L79
```

Each file is listed under a `── path` header. Files are sorted by path, grouped naturally by directory. Same format as single-file output, repeated per file.

## Design considerations

- Recurse into subdirectories, possibly with a configurable depth limit (`--depth N`)
- Respect `.gitignore` and index ignore patterns (same as `kdb check` / `kdb tree`)
- Skip unsupported file types
- `--json` output should include the file path per symbol (it already does for single files)
- Consider whether heading/markdown symbols should also be included or just code symbols
