---
id: 43
title: "symbols -s: include line number gutter in body output"
status: done
priority: low
labels:
  - enhancement
  - symbols
---

# ISS-0043 :: symbols -s: include line number gutter in body output

## Problem

`kdb symbols <file> -s <sym>` prints the symbol body without line numbers. Adding a line number gutter makes it easier to reference specific lines and matches the convention of other tools (`cat -n`, editor gutters).

## Desired behavior

```
$ kdb symbols src/resolve/rust.rs -s split_brace_group
  1126 | /// Split a use path at the outermost `{...}` brace group.
  1127 | fn split_brace_group(input: &str) -> Option<(&str, &str)> {
  1128 |     let start = input.find('{')?;
  ...
```

Line numbers should reflect the actual file lines, not 1-indexed from the body start.

## Stretch: include docstrings

When `-s <sym>` selects a code symbol, the output should include any preceding doc comment block (e.g. `///`, `/** */`, `#`, `"""`) as part of the body. Currently `extract_symbol_body` uses the tree-sitter node's byte span, which typically starts at the declaration keyword and excludes leading doc comments. Extending the span upward to capture attached documentation would make `-s` output self-contained.
