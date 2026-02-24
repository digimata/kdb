# kdb

A compiler and language server for markdown knowledge bases.

Treats a directory of markdown files like a codebase — headings are symbols, links are references, and broken links are compile errors.

If you've used Obsidian, the concept is familiar — but kdb is not tied to any app. It's an open standard for markdown knowledge bases with a compiler and language server that works in any editor.

![demo](docs/demo.gif)

## Knowledge = Code

| Code               | Knowledge                                   |
|---------------------|---------------------------------------------|
| Module              | Markdown file                               |
| Exported symbol     | Heading (`## Definition`)                   |
| Import / reference  | Link (`[text](file.md#heading)`)            |
| Compile error       | Broken link (target file/heading missing)   |
| Dead code           | Orphan file (nothing links to it)           |
| Public API          | Outline (heading tree of a file)            |
| Interface           | Template (expected structure for a category)|
| Dependency graph    | Link graph across files                     |

## Quickstart

1. Install the CLI:

```
cargo install --path .
```

2. Add the editor extension:

| Editor | Install |
|--------|---------|
| Zed    | Extensions panel → search "kdb" → Install |
| VSCode | Coming soon |

3. Init a kdb in your project:

```
cd my-notes
kdb init
```

This creates a `.kdb/` directory. You now get go-to-definition, autocomplete, hover previews, diagnostics, and document symbols in your editor.

## Link Syntax

Both standard markdown links and wikilinks are supported:

```markdown
[React Hooks](react/hooks.md#useEffect)
[[react/hooks#useEffect]]
```

## Commands

```
kdb check [--orphans]  # compile — report all errors/warnings (orphans listed with --orphans)
kdb outline <file>     # print heading tree
kdb symbols <path>     # print markdown/code symbols for one file
kdb refs <target>      # find inbound refs to a file or heading (`--json`, `--count`)
kdb orphans            # list orphan files
kdb stubs              # list empty stubs
kdb graph              # output dependency graph (dot format)
kdb graph --cluster    # detect clusters of related knowledge
kdb init               # initialize a kdb project (creates .kdb/config.toml)
kdb fmt                # generate/update code index headers (Rust, TS/JS, Python, Go)
kdb lsp                # start the language server (stdio)
```

## License

MIT
