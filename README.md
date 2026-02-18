# kdb

A compiler and language server for markdown knowledge bases.

Treats a directory of markdown files like a codebase — headings are symbols, links are references, and broken links are compile errors.

## Install

```
cargo install --path .
```

## Commands

```
kdb check              # compile — report all errors/warnings
kdb outline <file>     # print heading tree
kdb refs <file>#<head> # find all references to a heading
kdb orphans            # list orphan files
kdb stubs              # list empty stubs
kdb graph              # output dependency graph (dot format)
kdb graph --cluster    # detect clusters of related knowledge
kdb init               # initialize a kdb project (creates .kdb/config.toml)
kdb fmt                # normalize link formats, fix slugs
kdb lsp                # start the language server (stdio)
```

## Editor Support

The LSP provides go-to-definition, autocomplete, hover previews, diagnostics, and document symbols. A [Zed extension](extensions/zed) is included.

## Link Syntax

Both standard markdown links and wikilinks are supported:

```markdown
[React Hooks](react/hooks.md#useEffect)
[[react/hooks#useEffect]]
```

## License

MIT
