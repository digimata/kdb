# kdb

A structural index for codebases. CLI and language server.

kdb treats a project as a graph of symbols and references — whether those are markdown headings linked by wikilinks, or code functions connected by import statements. It parses every file into symbols, resolves cross-file references, and gives you a unified way to navigate the result.

## What it does

**Markdown**: headings are symbols, links are references, broken links are errors, orphan files are dead code.

**Code**: functions/classes/types are symbols, imports are references, resolved across files with language-aware import resolution.

Both sides share the same model — symbols, references, dependencies — and the same commands work on both.

### Supported languages

- Rust,
- TypeScript/JavaScript
- Python
- Go
- C#

## Quickstart

1. Install:

```
curl -fsSL https://kernl.sh/kdb/install | bash
```

Or from source (requires Rust):

```
cargo install --path .
```

2. Init in your project:

```
cd my-project
kdb init
```

This creates a `.kdb/` directory that marks the project root. All commands run relative to this boundary.

3. Add the editor extension (optional — for LSP features):

| Editor | Install |
|--------|---------|
| Zed    | Extensions panel → search "kdb" → Install |

## Commands

```
kdb symbols <path>      # list symbols in a file (headings, functions, types, etc.)
kdb refs <target>       # find inbound references to a file or heading
kdb refs <file> -s <s>  # find who imports a code symbol
kdb deps <file>         # list outbound dependencies (links, imports)
kdb check               # report broken links and orphan files
kdb index               # build or rebuild the project index
kdb tree [path]         # print filtered directory tree
kdb graph               # output dependency graph (dot format)
kdb fmt [path]          # generate/update code index headers
kdb lsp                 # start the language server (stdio)
```

### Markdown links

Both standard links and wikilinks are supported:

```markdown
[React Hooks](react/hooks.md#useEffect)
[[react/hooks#useEffect]]
```

### LSP

The language server provides go-to-definition, autocomplete, hover previews, diagnostics, and document symbols. It also advertises formatting for code files — in Zed you can chain it after language-native formatters:

```json
{
  "languages": {
    "Rust": {
      "formatter": [
        { "language_server": { "name": "rust-analyzer" } },
        { "language_server": { "name": "kdb" } }
      ],
      "format_on_save": "on"
    }
  }
}
```

## Development

Commits follow [conventional commits](https://www.conventionalcommits.org/). Changelog maintained with [git-cliff](https://git-cliff.org/) (`git cliff -o CHANGELOG.md`).

## License

MIT
