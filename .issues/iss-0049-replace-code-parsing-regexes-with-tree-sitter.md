---
id: 49
title: Replace code-parsing regexes with tree-sitter
status: proposed
priority: medium
labels:
  - refactor
---

# ISS-0049 :: Replace Code-Parsing Regexes with Tree-Sitter

## Intent

Several modules use regex to parse source code structure (imports, mod declarations, setup.py calls). These are fragile — they miss multi-line forms, match inside comments/strings, and duplicate logic across modules. We already have tree-sitter grammars for all affected languages in our deps.

## Scope

### Replace with tree-sitter

| File | Patterns | AST nodes |
|---|---|---|
| `resolve/tsjs.rs` | 5 import/require regexes | `import_statement`, `export_statement`, `call_expression` (require) |
| `resolve/rust.rs` | `MOD_RE`, `USE_RE` | `mod_item`, `use_declaration` |
| `resolve/python.rs` | `FIND_PACKAGES_WHERE_RE`, `PACKAGE_DIR_RE` | `call` nodes for `find_packages`/`find_namespace_packages` in setup.py |
| `deps/typescript.rs` | 3 import/export/require regexes | Same as resolve/tsjs.rs — deduplicate |
| `deps/rust.rs` | `MOD_RE`, `USE_RE` | Same as resolve/rust.rs — deduplicate |

### Keep as regex (fine)

| File | Patterns | Reason |
|---|---|---|
| `index/markdown.rs` | wikilink, md link, inline code | Simple markdown syntax, tree-sitter-md handles structure separately |
| `lsp/definition.rs` | md link, wikilink | Same — link target scanning |
| `lsp/hover.rs` | md link, wikilink | Same |

## Notes

- `deps/` patterns are duplicates of `resolve/` patterns. Once resolve uses tree-sitter, deps should share the same parsing.
- iss-0044 already covers the tsjs.rs portion. This issue tracks the full sweep.
- The `resolve/python.rs` regexes parse `setup.py` Python source — arguably the most fragile of the bunch.

## Why

Correctness and consolidation. Tree-sitter gives exact AST nodes, handles edge cases (multi-line, comments, strings), and we already pay for the grammars.
