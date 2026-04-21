---
path: projects/kdb/README.md
outline: |
  • kdb                           L16
    ◦ Overview                    L22
      ▪ Supported languages       L30
    ◦ Quickstart                  L38
    ◦ Commands                    L67
      ▪ Markdown links            L95
      ▪ Transclusion             L104
      ▪ LSP                      L115
    ◦ Development                L133
    ◦ License                    L137
---

# kdb

A structural index for codebases. CLI and language server.

kdb treats a project as a graph of symbols and references — whether those are markdown headings linked by wikilinks, or code functions connected by import statements. It parses every file into symbols, resolves cross-file references, and gives you a unified way to navigate the result.

## Overview

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
curl -fsSL https://kdb.digimata.dev/install | bash
```

Or from source (requires Rust):

```
cargo install --path .
```

2. Init in your workspace:

```
cd my-workspace
kdb init
```

This creates a `.kdb/` directory that marks the workspace root. All commands run relative to this boundary.

3. Add the editor extension (optional — for LSP features):

| Editor | Install |
|--------|---------|
| Zed    | Extensions panel → search "kdb" → Install |

## Commands

```
kdb root                # print absolute path of the workspace root
kdb outline <path>      # list the outline of a file (headings, functions, types, etc.)
kdb refs <target>       # find inbound references to a file or heading
kdb refs <file> -s <s>  # find who imports a code symbol
kdb deps <file>         # list outbound dependencies (links, imports)
kdb check               # report broken links, broken embeds, and orphan files
kdb render <file>       # resolve ![[]] embeds and print to stdout
kdb render --project <slug> | --all [--limit N]
                        # materialize .tasks/index.md + per-task files
                        # (in_progress + top-N open) from the relational layer
kdb tree [path]         # print filtered directory tree
kdb graph               # output dependency graph (dot format)
kdb fmt [path]          # generate/update code index headers
kdb lsp                 # start the language server (stdio)
kdb projects {list|add|edit|show}
                        # register and manage projects (slug, 2–6 char alias)
kdb tasks {list|add|edit|view|delete|done|park|reopen|label}
                         # manage tasks (per-project seq ids, priorities, statuses)
                         # `show` remains as an alias for `view`; `d` for `delete`
                         # tasks label {add|rm} <id> <label>... — attach/detach labels
kdb cycles {list|add|edit|show}
                        # manage time-boxed cycles (C-NN, start/end, status)
kdb labels {list|add|edit|show}
                        # manage free-form task tags (slug, name, optional color)
```

### Markdown links

Both standard links and wikilinks are supported:

```markdown
[React Hooks](react/hooks.md#useEffect)
[[react/hooks#useEffect]]
```

### Transclusion

Embed content from other files using Obsidian-style syntax:

```markdown
![[SOP.md#setup]]
![[kdb://lib/glossary.md]]
```

`kdb render` resolves embeds recursively and prints the result to stdout. Useful for composing documents from canonical sources at runtime.

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
