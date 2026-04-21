---
path: projects/kdb/CODEMAP.md
outline: |
  • Codemap                 L12
    ◦ Overview              L14
    ◦ Data flow             L22
    ◦ Key conventions       L31
    ◦ Modules               L40
    ◦ Testing              L112
---

# Codemap

## Overview

kdb is a compiler and language server for markdown knowledge bases. It treats a directory of `.md` files like a codebase — headings are exported symbols, links are imports, and broken references are compile errors. It also indexes code files for import resolution and dependency tracking.

**Index split**: `VaultIndex` handles markdown (files, headings, links, inbound maps). `CodeIndex` handles code (workspace packages, language caches, resolved imports). `WorkspaceIndex { vault, code }` wraps both. Most commands only need the vault; `deps` builds the full workspace index.

**Stateless vs stateful**: CLI commands are stateless (build index, do work, exit). The LSP is stateful — it caches the vault index and tracks open document buffers so unsaved edits are reflected immediately.

## Data flow

1. **Workspace discovery**: `WorkspaceContext::discover()` walks up looking for `.kdb/`, loads config + ignore patterns
2. **Markdown parsing**: `parse_markdown()` extracts headings and links from each `.md` file
3. **Vault indexing**: `VaultIndex::build()` assembles the file map and inbound link graphs
4. **Code indexing**: `CodeIndex::build()` scans code files and resolves imports per language
5. **Validation**: `VaultIndex::check()` resolves every link and embed, reports broken refs and orphans
6. **Resolution**: links resolve relative to their source file; wikilinks auto-append `.md`

## Key conventions

- All relative paths go through `normalize_rel_path()` — prevents path traversal outside root
- Ignore patterns use `globset`, configured via `[index].ignore` in `.kdb/config.toml`
- Workspace root is discovered by walking upward looking for `.kdb/` directory
- Heading anchors are GitHub-compatible slugs with numeric dedup suffixes
- The LSP uses `tower-lsp` over stdio; one `Backend` struct holds all shared state
- Tree-sitter is used for all code parsing (symbols, imports, index headers)

## Modules

```
src/
  main.rs           CLI entrypoint — clap parser, subcommand dispatch
  lib.rs            Crate root — public module declarations
  cmd.rs            CmdContext + subcommand implementations
  lang.rs           CodeLanguage enum + file-type detection
  tree.rs           Filtered tree rendering for `kdb tree`

  workspace/
    mod.rs          WorkspaceContext — root, config, ignore patterns
    root.rs         Root marker discovery — walks up looking for .kdb/
    config.rs       .kdb/config.toml parsing (index ignore patterns)
    ignore.rs       Ignore sets + always-ignored dirs (shared across modules)
    paths.rs        normalize_rel_path + rel path helpers
    discover.rs     File discovery walker (shared by index + tree + fmt)

  index/
    mod.rs          VaultIndex, CodeIndex, WorkspaceIndex — the core indexer
    markdown.rs     Markdown parser (headings, links, section bounds)
    refs.rs         Inbound reference queries for `kdb refs`
    deps.rs         Outbound dependency collection for `kdb deps`

  resolve/
    mod.rs          WorkspaceCaches + import resolution dispatch
    go.rs           Go import resolver
    python.rs       Python import resolver
    rust.rs         Rust use/mod resolver
    tsjs.rs         TypeScript/JavaScript import resolver

  deps/
    mod.rs          Standalone code deps extraction (non-index path)
    go.rs           Go dependency extraction
    python.rs       Python dependency extraction
    rust.rs         Rust dependency extraction
    typescript.rs   TypeScript/JavaScript dependency extraction
    utils.rs        Shared dep extraction helpers

  symbols/
    mod.rs          Symbol extraction dispatch + shared tree-sitter helpers
    extract/
      mod.rs        Extractor context struct
      go.rs         Go symbol extraction
      python.rs     Python symbol extraction
      rust.rs       Rust symbol extraction
      typescript.rs TypeScript/JavaScript symbol extraction
    query.rs        Symbol query API (collect_rows, collect_body_rows)
    display.rs      Text output formatting for symbol results
    tree.rs         Symbol tree building (qualified names, nesting)

  render/
    mod.rs          Public API — render_file, render_content
    include.rs      ![[target]] embed parsing (regex, types)
    resolve.rs      Recursive resolution engine (cycle detection, section extraction)

  fmt/
    mod.rs          Code file index header generation — workspace walker + rewriter
    preamble.rs     Language-specific preamble detection (doc comments, imports)

  lsp/
    mod.rs          LSP module root — re-exports and serve() entrypoint
    backend.rs      Server state (cached index, open docs) + LanguageServer impl
    completion.rs   Autocomplete for links and headings
    definition.rs   Go-to-definition for markdown/wikilinks
    diagnostics.rs  Broken link diagnostics (publish on open/change/close)
    formatting.rs   Code index header formatting via LSP
    hover.rs        Hover info for links (target heading preview)
    semantic_tokens.rs  Semantic token highlighting
    symbols.rs      Document symbol outline (heading tree)
```

## Testing

- `tests/cli.rs` — end-to-end CLI integration tests (all subcommands)
- `tests/index.rs` — vault/workspace indexer tests (parsing, resolution, validation, code imports)
- `tests/config.rs` — config loading tests
- `tests/fmt.rs` — code index header formatting tests
- `tests/lsp.rs` — LSP integration tests (diagnostics, completion, definition, hover, symbols)
- `tests/root.rs` — workspace root discovery tests
- `tests/render.rs` — transclusion resolution tests (embeds, recursion, cycles, check validation)
- `tests/symbols.rs` — symbol extraction tests (all languages)
- Run: `cargo test`
