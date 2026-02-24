# Codemap

## Overview

kdb is a compiler and language server for markdown knowledge bases. It treats a directory of `.md` files like a codebase — headings are exported symbols, links are imports, and broken references are compile errors.

## Architecture

```
CLI (src/main.rs)
 └─ cmd (src/cmd.rs) ─── dispatches subcommands
     ├─ init    → root, config
     ├─ check   → index (build + validate)
     ├─ outline → index (build + query)
     ├─ fmt     → fmt (walk + rewrite code files)
     └─ lsp     → lsp/backend (long-running server)

LSP (src/lsp/)
 └─ backend ─── holds cached VaultIndex + open document state
     ├─ completion  → index (heading/file completions)
     ├─ definition  → index (link → target resolution)
     ├─ diagnostics → index (broken link reporting)
     ├─ hover       → index (link preview on hover)
     └─ symbols     → index (heading outline for document)
```

**Core module**: `index` — everything flows through `VaultIndex`. CLI commands build a fresh index per invocation. The LSP caches one and updates it incrementally from editor events and file watcher notifications.

**Stateless vs stateful**: CLI commands are stateless (build index, do work, exit). The LSP is stateful — it caches the vault index and tracks open document buffers so unsaved edits are reflected immediately.

## Data flow

1. **Discovery**: walk the vault directory, respect `.kdb/config.toml` ignore patterns
2. **Parsing**: `parse_markdown()` extracts headings and links from each `.md` file
3. **Indexing**: `VaultIndex::build()` assembles the file map and inbound link graphs
4. **Validation**: `VaultIndex::check()` resolves every link, reports broken refs and orphans
5. **Resolution**: links resolve relative to their source file; wikilinks auto-append `.md`

## Key conventions

- All relative paths go through `normalize_rel_path()` — prevents path traversal outside root
- Ignore patterns use `globset`, configured via `[index].ignore` in `.kdb/config.toml`
- Project root is discovered by walking upward looking for `.kdb/` directory
- Heading anchors are GitHub-compatible slugs with numeric dedup suffixes
- The LSP uses `tower-lsp` over stdio; one `Backend` struct holds all shared state

## Modules

```
src/
  main.rs           CLI entrypoint — clap parser, subcommand dispatch
  cmd.rs            Subcommand implementations (init, check, outline, fmt, lsp)
  config.rs         .kdb/config.toml parsing (index ignore patterns)
  root.rs           Project root discovery — walks up looking for .kdb/

  index/
    mod.rs          Markdown parser, vault indexer, link resolver — the core module

  fmt/
    mod.rs          Code file index header generation — workspace walker + file rewriter
    languages.rs    Tree-sitter symbol extraction for Rust, TS/JS, Python, Go

  lsp/
    mod.rs          LSP module root — re-exports and serve() entrypoint
    backend.rs      Server state (cached index, open docs) + LanguageServer trait impl
    completion.rs   Autocomplete for links and headings
    definition.rs   Go-to-definition for markdown/wikilinks
    diagnostics.rs  Broken link diagnostics (publish on open/change/close)
    hover.rs        Hover info for links (target heading preview)
    symbols.rs      Document symbol outline (heading tree)
```

## Testing

- `tests/cli.rs` — end-to-end CLI integration tests (init, check, outline)
- `tests/index.rs` — vault indexer unit tests (parsing, resolution, validation)
- Run: `cargo test`
